// Extensions in JavaScript (see src/extensions.rs, EXTENSIONS.md), run by
// F2 itself started again as their host (F2_EXTENSION_HOST): macOS's own
// JavaScriptCore, nothing to install. Shut in before the script runs, in
// macOS's sandbox for pure computation (no files, no network, no other
// processes): all it can do is talk to F2 over its stdin and stdout,
// which does what it asks if its manifest says it may (fetch from its
// hosts, its storage), and shows what it says.
//
// On stdin, first a line of JSON: the API (`prelude`, src/js_host.js),
// the script (and its file's name, `main`), and what's wanted of it
// (`command`, `args`, `query`); then
// F2's replies, a line each ({"reply": id, "result" or "error"}). On
// stdout, a line each: {"say": message} for F2 to show, or {"call": id,
// "op": …} for it to do. What the script logs goes to stderr.

#import <Foundation/Foundation.h>
#import <JavaScriptCore/JavaScriptCore.h>
#include <fcntl.h>
#include <poll.h>
#include <sandbox.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <errno.h>
#include <unistd.h>

// Lines read from stdin, as they come (the first is long: the script).
static NSMutableData *pending;
static bool closed;

// The next whole line, if there is one (without waiting).
static NSString *next_line(void) {
    const char *bytes = pending.bytes;
    for (NSUInteger i = 0; i < pending.length; i++) {
        if (bytes[i] != '\n') continue;
        NSString *line = [[NSString alloc] initWithBytes:bytes length:i encoding:NSUTF8StringEncoding];
        [pending replaceBytesInRange:NSMakeRange(0, i + 1) withBytes:NULL length:0];
        return line ?: @"";
    }
    return nil;
}

// More read from stdin, waiting at most `ms` (-1: till there's some).
static void read_more(int ms) {
    struct pollfd fd = {0, POLLIN, 0};
    if (poll(&fd, 1, ms) <= 0) return;
    char chunk[65536];
    ssize_t n = read(0, chunk, sizeof chunk);
    if (n <= 0) {
        closed = true;
        return;
    }
    [pending appendBytes:chunk length:(NSUInteger)n];
}

static void write_line(int fd, NSString *text) {
    NSData *data = [[text stringByAppendingString:@"\n"] dataUsingEncoding:NSUTF8StringEncoding];
    const char *bytes = data.bytes;
    NSUInteger left = data.length;
    while (left > 0) {
        ssize_t n = write(fd, bytes, left);
        if (n <= 0) return;
        bytes += n;
        left -= (NSUInteger)n;
    }
}

// Shut in: macOS's sandbox for pure computation, and then checked, so a
// script never runs outside it (a file can't be opened, nor a connection
// made).
static bool shut_in(void) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    char *error = NULL;
    if (sandbox_init(kSBXProfilePureComputation, SANDBOX_NAMED, &error) != 0) {
        if (error) {
            fprintf(stderr, "sandbox: %s\n", error);
            sandbox_free_error(error);
        }
        return false;
    }
#pragma clang diagnostic pop
    int file = open("/etc/hosts", O_RDONLY);
    if (file >= 0) {
        close(file);
        return false;
    }
    // (A socket it may make; connecting it, even to this Mac, it may not:
    // the sandbox's EPERM, not a closed port's refusal.)
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    if (sock >= 0) {
        struct sockaddr_in to = {.sin_family = AF_INET, .sin_port = htons(9), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        int connected = connect(sock, (struct sockaddr *)&to, sizeof to);
        int why = errno;
        close(sock);
        if (connected == 0 || why != EPERM) return false;
    }
    return true;
}

int nf_js_host(void) {
    @autoreleasepool {
        pending = [NSMutableData data];
        NSString *first = nil;
        while (!(first = next_line()) && !closed) read_more(-1);
        NSDictionary *start = first ? [NSJSONSerialization JSONObjectWithData:[first dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil] : nil;
        if (![start isKindOfClass:NSDictionary.class]) return 2;

        JSContext *js = [JSContext new];
        __block bool done = false;
        __block int code = 0;
        // Timers by id: when each is due.
        NSMutableDictionary<NSNumber *, NSDate *> *timers = [NSMutableDictionary dictionary];
        js[@"__f2_post"] = ^(NSString *line) {
          write_line(1, line);
        };
        js[@"__f2_log"] = ^(NSString *line) {
          write_line(2, line);
        };
        js[@"__f2_timer_set"] = ^(NSNumber *id, NSNumber *ms) {
          timers[id] = [NSDate dateWithTimeIntervalSinceNow:MAX(0, ms.doubleValue) / 1000];
        };
        js[@"__f2_timer_clear"] = ^(NSNumber *id) {
          [timers removeObjectForKey:id];
        };
        js[@"__f2_done"] = ^(JSValue *error) {
          done = true;
          if (!error.isNull && !error.isUndefined) {
              NSData *said = [NSJSONSerialization dataWithJSONObject:@{@"say": @{@"error": error.toString, @"loading": @NO}} options:0 error:nil];
              write_line(1, [[NSString alloc] initWithData:said encoding:NSUTF8StringEncoding]);
              code = 1;
          }
        };
        // (What was thrown that nothing caught: to its output, and kept,
        // to say if it was its script's own, not running at all.)
        __block JSValue *thrown = nil;
        js.exceptionHandler = ^(JSContext *context, JSValue *exception) {
          (void)context;
          thrown = exception;
          write_line(2, [NSString stringWithFormat:@"%@ (line %@)", exception, exception[@"line"]]);
        };
        // (What's read from disk lazily, read now, before it can't be: the
        // time zone, and ICU's data for dates and numbers.)
        [js evaluateScript:@"new Date().toLocaleString(); (1234.5).toLocaleString(); 'A'.localeCompare('b'); /x/u.test('x'); JSON.stringify({})"];
        [NSTimeZone resetSystemTimeZone];
        (void)NSTimeZone.localTimeZone;

        if (!shut_in()) {
            write_line(1, @"{\"say\": {\"error\": \"F2 couldn't shut the extension in its sandbox, so it didn't run it.\", \"loading\": false}}");
            return 3;
        }

        // (Its settings, for `f2.preferences`.)
        js[@"__f2_preferences"] = [start[@"preferences"] isKindOfClass:NSDictionary.class] ? start[@"preferences"] : @{};
        [js evaluateScript:start[@"prelude"] ?: @""];
        // (Its file's name in what's said of an error: "index.js:12".)
        NSString *main = [start[@"main"] isKindOfClass:NSString.class] ? [start[@"main"] lastPathComponent] : @"index.js";
        NSString *file = [main stringByAddingPercentEncodingWithAllowedCharacters:NSCharacterSet.URLPathAllowedCharacterSet] ?: @"index.js";
        thrown = nil;
        [js evaluateScript:start[@"script"] ?: @"" withSourceURL:[NSURL URLWithString:[@"extension:///" stringByAppendingString:file]]];
        if (thrown) {
            NSString *line = [thrown[@"line"] isNumber] ? [NSString stringWithFormat:@" (%@:%@)", main, [thrown[@"line"] toNumber]] : @"";
            NSData *said = [NSJSONSerialization dataWithJSONObject:@{@"say": @{@"error": [thrown.toString stringByAppendingString:line], @"loading": @NO}} options:0 error:nil];
            write_line(1, [[NSString alloc] initWithData:said encoding:NSUTF8StringEncoding]);
            return 1;
        }
        JSValue *started = [js[@"__f2_start"] callWithArguments:@[ start[@"command"] ?: @"", start[@"args"] ?: @{}, start[@"query"] ?: @"" ]];
        [started invokeMethod:@"then" withArguments:@[ js[@"__f2_done"], js[@"__f2_failed"] ]];

        // Till the script's done (or F2's gone): F2's replies, and timers
        // as they come due. (JavaScriptCore runs what they let go on, its
        // promises, as each call into it returns.)
        while (!done && !closed) {
            NSString *line;
            while (!done && (line = next_line())) {
                NSDictionary *reply = [NSJSONSerialization JSONObjectWithData:[line dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
                if (![reply isKindOfClass:NSDictionary.class] || !reply[@"reply"]) continue;
                [js[@"__f2_reply"] callWithArguments:@[ reply[@"reply"], line ]];
            }
            if (done) break;
            NSDate *soonest = nil;
            NSNumber *due = nil;
            for (NSNumber *id in timers) {
                if (!soonest || [timers[id] compare:soonest] == NSOrderedAscending) {
                    soonest = timers[id];
                    due = id;
                }
            }
            double wait = soonest ? soonest.timeIntervalSinceNow : -1;
            if (due && wait <= 0) {
                [timers removeObjectForKey:due];
                [js[@"__f2_timer"] callWithArguments:@[ due ]];
                continue;
            }
            read_more(soonest ? (int)ceil(wait * 1000) : -1);
        }
        return code;
    }
}
