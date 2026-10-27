// A request made for an extension's script (see src/extensions.rs): only
// over HTTPS, only to the hosts its manifest names (redirects too), in a
// session of its own that keeps nothing (no cookies, no cache, nothing of
// F2's), at most so much of an answer, in so long.

#import <Foundation/Foundation.h>

// `host` is one of `allowed` ("api.github.com", or "*.github.com": any
// under it, and it).
static bool nf_host_allowed(NSString *host, NSArray<NSString *> *allowed) {
    host = host.lowercaseString;
    for (NSString *entry in allowed) {
        NSString *e = entry.lowercaseString;
        if ([e hasPrefix:@"*."]) {
            NSString *base = [e substringFromIndex:2];
            if ([host isEqual:base] || [host hasSuffix:[@"." stringByAppendingString:base]]) return true;
        } else if ([host isEqual:e]) {
            return true;
        }
    }
    return false;
}

@interface NFFetchGuard : NSObject <NSURLSessionTaskDelegate, NSURLSessionDataDelegate>
@property(copy) NSArray<NSString *> *allowed;
@property NSUInteger most;
@property(strong) NSMutableData *body;
@property(strong) NSHTTPURLResponse *response;
@property(strong) NSError *error;
@property(copy) NSString *refused;
@property(strong) dispatch_semaphore_t done;
@end

@implementation NFFetchGuard
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request completionHandler:(void (^)(NSURLRequest *))completionHandler {
    NSURL *to = request.URL;
    if ([to.scheme.lowercaseString isEqual:@"https"] && nf_host_allowed(to.host ?: @"", self.allowed)) {
        completionHandler(request);
    } else {
        self.refused = [NSString stringWithFormat:@"Redirected to %@, which its manifest doesn't allow.", to.host ?: to.absoluteString];
        completionHandler(nil);
    }
}

- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveResponse:(NSURLResponse *)response completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
    self.response = (NSHTTPURLResponse *)response;
    completionHandler(NSURLSessionResponseAllow);
}

- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveData:(NSData *)data {
    if (self.body.length + data.length > self.most) {
        self.refused = [NSString stringWithFormat:@"The answer is bigger than %lu MB.", (unsigned long)(self.most >> 20)];
        [task cancel];
        return;
    }
    [self.body appendData:data];
}

- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
    self.error = error;
    dispatch_semaphore_signal(self.done);
}
@end

// `request` (JSON: url, method, headers, body), for hosts `allowed` (JSON
// array). The answer, as JSON (malloc'd): {status, url, headers, body}, or
// {error}. Waits for it: not on the main thread.
char *nf_extension_fetch(const char *request, const char *allowed) {
    @autoreleasepool {
        NSDictionary *r = [NSJSONSerialization JSONObjectWithData:[@(request) dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
        NSArray *hosts = [NSJSONSerialization JSONObjectWithData:[@(allowed) dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
        NSDictionary *answer;
        NSURL *url = [r isKindOfClass:NSDictionary.class] && [r[@"url"] isKindOfClass:NSString.class] ? [NSURL URLWithString:r[@"url"]] : nil;
        if (!url || ![hosts isKindOfClass:NSArray.class]) {
            answer = @{@"error": @"That isn't a URL."};
        } else if (![url.scheme.lowercaseString isEqual:@"https"]) {
            answer = @{@"error": @"Only https:// addresses can be fetched."};
        } else if (!nf_host_allowed(url.host ?: @"", hosts)) {
            answer = @{@"error": [NSString stringWithFormat:@"%@ isn't one of the hosts its manifest allows.", url.host ?: @"That host"]};
        } else {
            NSMutableURLRequest *req = [NSMutableURLRequest requestWithURL:url];
            req.HTTPMethod = [r[@"method"] isKindOfClass:NSString.class] ? [r[@"method"] uppercaseString] : @"GET";
            req.timeoutInterval = 30;
            NSDictionary *headers = [r[@"headers"] isKindOfClass:NSDictionary.class] ? r[@"headers"] : @{};
            for (NSString *name in headers) {
                if ([headers[name] isKindOfClass:NSString.class]) [req setValue:headers[name] forHTTPHeaderField:name];
            }
            if ([r[@"body"] isKindOfClass:NSString.class]) req.HTTPBody = [r[@"body"] dataUsingEncoding:NSUTF8StringEncoding];
            NSURLSessionConfiguration *config = NSURLSessionConfiguration.ephemeralSessionConfiguration;
            config.HTTPCookieStorage = nil;
            config.URLCache = nil;
            config.timeoutIntervalForResource = 60;
            NFFetchGuard *guard = [NFFetchGuard new];
            guard.allowed = hosts;
            guard.most = 20u << 20;
            guard.body = [NSMutableData data];
            guard.done = dispatch_semaphore_create(0);
            NSURLSession *session = [NSURLSession sessionWithConfiguration:config delegate:guard delegateQueue:nil];
            [[session dataTaskWithRequest:req] resume];
            dispatch_semaphore_wait(guard.done, DISPATCH_TIME_FOREVER);
            [session finishTasksAndInvalidate];
            if (guard.refused) {
                answer = @{@"error": guard.refused};
            } else if (guard.error) {
                answer = @{@"error": guard.error.localizedDescription ?: @"It couldn't be fetched."};
            } else {
                NSString *body = [[NSString alloc] initWithData:guard.body encoding:NSUTF8StringEncoding]
                    ?: [[NSString alloc] initWithData:guard.body encoding:NSISOLatin1StringEncoding] ?: @"";
                NSMutableDictionary *names = [NSMutableDictionary dictionary];
                [guard.response.allHeaderFields enumerateKeysAndObjectsUsingBlock:^(id key, id value, BOOL *stop) {
                  (void)stop;
                  names[[key description].lowercaseString] = [value description];
                }];
                answer = @{@"status": @(guard.response.statusCode), @"url": guard.response.URL.absoluteString ?: url.absoluteString, @"headers": names, @"body": body};
            }
        }
        NSData *json = [NSJSONSerialization dataWithJSONObject:answer options:0 error:nil];
        return strndup(json.bytes, json.length);
    }
}
