// What Get Info and the list show and change through the system: hidden
// extensions, Locked, comments, Spotlight's More Info and Last Opened, the
// default app for a type, custom icons, iCloud downloads, Smart Folders.
//
// Calls that can fail return NULL on success, else a malloc'd message.

#import <AppKit/AppKit.h>
#import <CoreServices/CoreServices.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <sys/xattr.h>

static NSURL *url(const char *path) {
    return [NSURL fileURLWithPath:@(path)];
}

static char *message(NSError *error) {
    return error ? strdup(error.localizedDescription.UTF8String) : NULL;
}

bool nf_has_hidden_extension(const char *path) {
    @autoreleasepool {
        NSNumber *hidden = nil;
        [url(path) getResourceValue:&hidden forKey:NSURLHasHiddenExtensionKey error:nil];
        return hidden.boolValue;
    }
}

char *nf_set_hidden_extension(const char *path, bool hidden) {
    @autoreleasepool {
        NSError *error = nil;
        [url(path) setResourceValue:@(hidden) forKey:NSURLHasHiddenExtensionKey error:&error];
        return message(error);
    }
}

bool nf_is_locked(const char *path) {
    @autoreleasepool {
        NSNumber *locked = nil;
        [url(path) getResourceValue:&locked forKey:NSURLIsUserImmutableKey error:nil];
        return locked.boolValue;
    }
}

char *nf_set_locked(const char *path, bool locked) {
    @autoreleasepool {
        NSError *error = nil;
        [url(path) setResourceValue:@(locked) forKey:NSURLIsUserImmutableKey error:&error];
        return message(error);
    }
}

// 0: not in the cloud; 1: here and current; 2: not downloaded; 3:
// downloading; 4: uploading.
int nf_cloud_state(const char *path) {
    @autoreleasepool {
        NSDictionary *v = [url(path) resourceValuesForKeys:@[
            NSURLIsUbiquitousItemKey, NSURLUbiquitousItemDownloadingStatusKey, NSURLUbiquitousItemIsDownloadingKey,
            NSURLUbiquitousItemIsUploadingKey
        ] error:nil];
        if (![v[NSURLIsUbiquitousItemKey] boolValue]) return 0;
        if ([v[NSURLUbiquitousItemIsDownloadingKey] boolValue]) return 3;
        if ([v[NSURLUbiquitousItemIsUploadingKey] boolValue]) return 4;
        NSString *status = v[NSURLUbiquitousItemDownloadingStatusKey];
        return [status isEqualToString:NSURLUbiquitousItemDownloadingStatusNotDownloaded] ? 2 : 1;
    }
}

char *nf_download(const char *path) {
    @autoreleasepool {
        NSError *error = nil;
        [NSFileManager.defaultManager startDownloadingUbiquitousItemAtURL:url(path) error:&error];
        return message(error);
    }
}

char *nf_evict(const char *path) {
    @autoreleasepool {
        NSError *error = nil;
        [NSFileManager.defaultManager evictUbiquitousItemAtURL:url(path) error:&error];
        return message(error);
    }
}

static MDItemRef md_item(const char *path) {
    return MDItemCreate(NULL, (__bridge CFStringRef) @(path));
}

// When it was last opened, as Spotlight knows it (seconds since 1970), or -1.
double nf_last_opened(const char *path) {
    @autoreleasepool {
        MDItemRef item = md_item(path);
        if (!item) return -1;
        NSDate *date = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemLastUsedDate));
        CFRelease(item);
        return date ? date.timeIntervalSince1970 : -1;
    }
}

// Get Info's More Info: "label\tvalue" lines (dimensions, duration, pages,
// version, where it came from...), or NULL.
char *nf_more_info(const char *path) {
    @autoreleasepool {
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        MDItemRef item = md_item(path);
        if (item) {
            NSNumber *w = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemPixelWidth));
            NSNumber *h = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemPixelHeight));
            if (w && h) [lines addObject:[NSString stringWithFormat:@"Dimensions\t%@ × %@", w, h]];
            NSNumber *duration = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemDurationSeconds));
            if (duration.doubleValue > 0) {
                long s = lround(duration.doubleValue);
                NSString *text = s >= 3600 ? [NSString stringWithFormat:@"%ld:%02ld:%02ld", s / 3600, s / 60 % 60, s % 60]
                                           : [NSString stringWithFormat:@"%02ld:%02ld", s / 60, s % 60];
                [lines addObject:[@"Duration\t" stringByAppendingString:text]];
            }
            NSNumber *pages = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemNumberOfPages));
            if (pages.intValue > 0) [lines addObject:[NSString stringWithFormat:@"Pages\t%@", pages]];
            NSArray *codecs = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemCodecs));
            if (codecs.count) [lines addObject:[@"Codecs\t" stringByAppendingString:[codecs componentsJoinedByString:@", "]]];
            NSArray *from = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemWhereFroms));
            if (from.count) [lines addObject:[@"Where from\t" stringByAppendingString:[from componentsJoinedByString:@", "]]];
            NSDate *opened = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemLastUsedDate));
            if (opened) {
                NSString *when = [NSDateFormatter localizedStringFromDate:opened
                                                                dateStyle:NSDateFormatterMediumStyle
                                                                timeStyle:NSDateFormatterShortStyle];
                [lines addObject:[@"Last opened\t" stringByAppendingString:when]];
            }
            CFRelease(item);
        }
        NSBundle *bundle = [NSBundle bundleWithPath:@(path)];
        NSString *version = [bundle objectForInfoDictionaryKey:@"CFBundleShortVersionString"];
        if (version) [lines addObject:[@"Version\t" stringByAppendingString:version]];
        return lines.count ? strdup([lines componentsJoinedByString:@"\n"].UTF8String) : NULL;
    }
}

// The Spotlight comment Get Info shows and edits.
char *nf_comment(const char *path) {
    @autoreleasepool {
        MDItemRef item = md_item(path);
        if (!item) return NULL;
        NSString *comment = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemFinderComment));
        CFRelease(item);
        return comment.length ? strdup(comment.UTF8String) : NULL;
    }
}

char *nf_set_comment(const char *path, const char *text) {
    @autoreleasepool {
        const char *attr = "com.apple.metadata:kMDItemFinderComment";
        if (!*text) {
            removexattr(path, attr, 0);
            return NULL;
        }
        NSData *data = [NSPropertyListSerialization dataWithPropertyList:@(text)
                                                                  format:NSPropertyListBinaryFormat_v1_0
                                                                 options:0
                                                                   error:nil];
        if (setxattr(path, attr, data.bytes, data.length, 0, 0) != 0) return strdup(strerror(errno));
        return NULL;
    }
}

// Open every file of `path`'s type with `app` (Get Info's Change All).
char *nf_set_default_app(const char *path, const char *app) {
    @autoreleasepool {
        UTType *type = nil;
        [url(path) getResourceValue:&type forKey:NSURLContentTypeKey error:nil];
        if (!type) return strdup("The kind of this item isn't known.");
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        __block NSError *failed = nil;
        [NSWorkspace.sharedWorkspace setDefaultApplicationAtURL:url(app)
                                              toOpenContentType:type
                                              completionHandler:^(NSError *error) {
                                                failed = error;
                                                dispatch_semaphore_signal(done);
                                              }];
        dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC));
        return message(failed);
    }
}

// The app that opens `path` (malloc'd), or NULL.
char *nf_default_app(const char *path) {
    @autoreleasepool {
        NSURL *app = [NSWorkspace.sharedWorkspace URLForApplicationToOpenURL:url(path)];
        return app ? strdup(app.path.UTF8String) : NULL;
    }
}

// A custom icon from the image on the pasteboard (Get Info's paste), or,
// with `clear`, back to the usual one.
char *nf_set_icon(const char *path, bool clear) {
    @autoreleasepool {
        NSImage *image = nil;
        if (!clear) {
            image = [[NSImage alloc] initWithPasteboard:NSPasteboard.generalPasteboard];
            if (!image) return strdup("There's no image on the clipboard to use as the icon.");
        }
        if (![NSWorkspace.sharedWorkspace setIcon:image forFile:@(path) options:0]) {
            return strdup("The icon couldn't be changed.");
        }
        return NULL;
    }
}

// A Smart Folder (.savedSearch): its Spotlight query, then the folders it
// searches, one per line.
char *nf_saved_search(const char *path) {
    @autoreleasepool {
        NSDictionary *plist = [NSDictionary dictionaryWithContentsOfFile:@(path)];
        NSString *query = plist[@"RawQuery"];
        if (!query.length) return NULL;
        NSMutableArray<NSString *> *lines = [NSMutableArray arrayWithObject:query];
        NSArray *scopes = plist[@"SearchCriteria"][@"FXScopeArrayOfPaths"];
        for (id scope in scopes) {
            if ([scope isKindOfClass:NSString.class] && [scope hasPrefix:@"/"]) [lines addObject:scope];
        }
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}

// Save a Smart Folder for a Spotlight query over `scope` (or the whole Mac).
char *nf_write_saved_search(const char *path, const char *query, const char *scope) {
    @autoreleasepool {
        NSArray *scopes = *scope ? @[ @(scope) ] : @[ @"kMDQueryScopeComputer" ];
        NSDictionary *plist = @{
            @"CompatibleVersion" : @1,
            @"RawQuery" : @(query),
            @"RawQueryDict" : @{@"FinderFilesOnly" : @YES, @"RawQuery" : @(query), @"SearchScopes" : scopes, @"UserFilesOnly" : @YES},
            @"SearchCriteria" : @{@"FXScopeArrayOfPaths" : scopes},
        };
        NSError *error = nil;
        NSData *data = [NSPropertyListSerialization dataWithPropertyList:plist format:NSPropertyListXMLFormat_v1_0 options:0 error:&error];
        if (!data) return message(error);
        [data writeToFile:@(path) options:NSDataWritingAtomic error:&error];
        return message(error);
    }
}
