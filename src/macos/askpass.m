// ssh's questions (a password, a key's passphrase, a server's key not
// known yet), asked in a dialog: ssh runs F2 itself as its SSH_ASKPASS
// (see sftp.rs), which shows this and prints the answer. No Dock icon.
// A locked disk image's password is asked the same way (disk_images.rs).

#import <AppKit/AppKit.h>

// `prompt` as ssh put it; `kind`: SSH_ASKPASS_PROMPT ("confirm": a yes or
// no, "none": only to be seen; "unlock": a disk image's password, `prompt`
// its title, then what to say under it). The answer (malloc'd), or NULL:
// cancelled.
char *nf_askpass(const char *prompt, const char *kind) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSApp.activationPolicy = NSApplicationActivationPolicyAccessory;
        NSString *text = [@(prompt ?: "") stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
        NSString *lower = text.lowercaseString;
        NSString *how = kind ? @(kind) : @"";
        bool hostKey = [lower containsString:@"(yes/no"];
        bool confirm = hostKey || [how isEqual:@"confirm"];
        bool info = [how isEqual:@"none"];
        bool unlock = [how isEqual:@"unlock"];
        bool secret = unlock || (!confirm && !info
            && ([lower containsString:@"password"] || [lower containsString:@"passphrase"] || [lower containsString:@"pin"] || [lower containsString:@"verification code"]));

        NSAlert *alert = [NSAlert new];
        NSArray<NSString *> *lines = [text componentsSeparatedByString:@"\n"];
        if (hostKey) {
            // ("The authenticity of host 'name (address)' can't be
            // established. ED25519 key fingerprint is SHA256:… Are you
            // sure…?")
            NSString *host = @"This server";
            NSRange a = [text rangeOfString:@"host '"];
            if (a.location != NSNotFound) {
                NSString *rest = [text substringFromIndex:NSMaxRange(a)];
                NSRange b = [rest rangeOfString:@"'"];
                if (b.location != NSNotFound) host = [rest substringToIndex:b.location];
                // ("name (address)": the name.)
                NSRange paren = [host rangeOfString:@" ("];
                if (paren.location != NSNotFound) host = [host substringToIndex:paren.location];
            }
            alert.messageText = [NSString stringWithFormat:@"Connect to %@?", host];
            NSMutableArray *keep = [NSMutableArray array];
            for (NSString *l in lines) {
                if ([l containsString:@"key fingerprint is"]) [keep addObject:l];
            }
            alert.informativeText = [NSString stringWithFormat:@"F2 hasn't connected to it before, so it can't tell it's the real one.\n\n%@\n\nIf you're not sure, ask whoever runs it whether the fingerprint matches.",
                [keep componentsJoinedByString:@"\n"]];
            [alert addButtonWithTitle:@"Connect"];
        } else if (unlock) {
            alert.messageText = lines.firstObject ?: @"";
            if (lines.count > 1) alert.informativeText = [[lines subarrayWithRange:NSMakeRange(1, lines.count - 1)] componentsJoinedByString:@"\n"];
            [alert addButtonWithTitle:@"Open"];
        } else {
            alert.messageText = lines.lastObject.length ? lines.lastObject : @"SSH";
            if (lines.count > 1) alert.informativeText = [[lines subarrayWithRange:NSMakeRange(0, lines.count - 1)] componentsJoinedByString:@"\n"];
            [alert addButtonWithTitle:info ? @"OK" : confirm ? @"Allow" : @"Connect"];
        }
        if (!info) [alert addButtonWithTitle:@"Cancel"];
        NSTextField *field = nil;
        if (!confirm && !info) {
            field = secret ? [[NSSecureTextField alloc] initWithFrame:NSMakeRect(0, 0, 280, 24)] : [[NSTextField alloc] initWithFrame:NSMakeRect(0, 0, 280, 24)];
            alert.accessoryView = field;
            alert.window.initialFirstResponder = field;
        }
        if (@available(macOS 14, *)) {
            [NSApp activate];
        } else {
            [NSApp activateIgnoringOtherApps:YES];
        }
        [alert.window setLevel:NSFloatingWindowLevel];
        // (Again once it's up: macOS lets an app come forward only once the
        // one in front lets it, which F2 does as it starts this, maybe a
        // moment after the first try. See `nf_yield_to`.)
        for (int i = 1; i <= 3; i++) {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, i * 250 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
                if (NSApp.isActive) return;
                if (@available(macOS 14, *)) {
                    [NSApp activate];
                } else {
                    [NSApp activateIgnoringOtherApps:YES];
                }
                [alert.window makeKeyAndOrderFront:nil];
            });
        }
        NSModalResponse r = [alert runModal];
        if (r != NSAlertFirstButtonReturn) return NULL;
        if (hostKey) return strdup("yes");
        if (!field) return strdup("");
        return strdup(field.stringValue.UTF8String ?: "");
    }
}
