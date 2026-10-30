// What Finder does to files through AppKit: the Trash, the pasteboard,
// aliases, tags, the apps that open a file, and ejecting volumes.
//
// Calls that can fail return NULL on success, else a malloc'd message.

#import <AppKit/AppKit.h>
#import <CommonCrypto/CommonDigest.h>
#import <PDFKit/PDFKit.h>
#include <errno.h>
#include <sys/attr.h>
#include <sys/xattr.h>

static char *message(NSError *error) {
    return error ? strdup(error.localizedDescription.UTF8String) : NULL;
}

static NSURL *url(const char *path) {
    return [NSURL fileURLWithPath:@(path)];
}

// Move to the Trash; `*trashed` gets where it went (malloc'd), to put it
// back from.
// `*denied`: it wasn't this user's to move (an app an administrator
// installed, say), which Finder can do with an administrator's password.
char *nf_trash(const char *path, char **trashed, bool *denied) {
    @autoreleasepool {
        NSError *error = nil;
        NSURL *resulting = nil;
        [NSFileManager.defaultManager trashItemAtURL:url(path) resultingItemURL:&resulting error:&error];
        *trashed = resulting ? strdup(resulting.path.UTF8String) : NULL;
        NSError *under = error.userInfo[NSUnderlyingErrorKey];
        *denied = error
            && (([error.domain isEqual:NSCocoaErrorDomain] && (error.code == NSFileWriteNoPermissionError || error.code == NSFileReadNoPermissionError))
                || ([under.domain isEqual:NSPOSIXErrorDomain] && (under.code == EACCES || under.code == EPERM)));
        return message(error);
    }
}

// The file's tags, one per line as "name\tcolor" (Finder's color numbers:
// 0 none, 1 gray, 2 green, 3 purple, 4 blue, 5 yellow, 6 red, 7 orange).
// They're stored in an extended attribute as "name\ncolor" strings.
char *nf_tags(const char *path) {
    @autoreleasepool {
        const char *attr = "com.apple.metadata:_kMDItemUserTags";
        ssize_t size = getxattr(path, attr, NULL, 0, 0, XATTR_NOFOLLOW);
        if (size <= 0) return NULL;
        NSMutableData *data = [NSMutableData dataWithLength:size];
        if (getxattr(path, attr, data.mutableBytes, size, 0, XATTR_NOFOLLOW) != size) return NULL;
        NSArray *tags = [NSPropertyListSerialization propertyListWithData:data options:0 format:nil error:nil];
        if (![tags isKindOfClass:NSArray.class] || !tags.count) return NULL;
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        for (NSString *tag in tags) {
            if (![tag isKindOfClass:NSString.class]) continue;
            NSArray<NSString *> *parts = [tag componentsSeparatedByString:@"\n"];
            NSString *color = parts.count > 1 ? parts[1] : @"0";
            [lines addObject:[NSString stringWithFormat:@"%@\t%@", parts[0], color]];
        }
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}

// Set the file's tags (names one per line; Finder's own names get their
// colors).
char *nf_set_tags(const char *path, const char *names) {
    @autoreleasepool {
        NSString *joined = @(names);
        NSArray *list = joined.length ? [joined componentsSeparatedByString:@"\n"] : @[];
        NSError *error = nil;
        [url(path) setResourceValue:list forKey:NSURLTagNamesKey error:&error];
        return message(error);
    }
}

// Where an alias (or symlink) leads; NULL if it doesn't resolve.
char *nf_resolve_alias(const char *path) {
    @autoreleasepool {
        NSURL *target = [NSURL URLByResolvingAliasFileAtURL:url(path) options:NSURLBookmarkResolutionWithoutUI error:nil];
        return target ? strdup(target.path.UTF8String) : NULL;
    }
}

// Files on the general pasteboard, as Finder's Copy puts them there.
void nf_copy_files(const char *const *paths, size_t count) {
    @autoreleasepool {
        NSMutableArray<NSURL *> *urls = [NSMutableArray array];
        for (size_t i = 0; i < count; i++) [urls addObject:url(paths[i])];
        [NSPasteboard.generalPasteboard clearContents];
        [NSPasteboard.generalPasteboard writeObjects:urls];
    }
}

void nf_copy_text(const char *text) {
    @autoreleasepool {
        [NSPasteboard.generalPasteboard clearContents];
        [NSPasteboard.generalPasteboard setString:@(text) forType:NSPasteboardTypeString];
    }
}

// Copy a password: marked as password managers mark theirs (concealed,
// and not to be kept), so clipboard histories, F2's too, leave it out; and
// taken off the clipboard after `seconds`, unless something else has been
// copied since.
void nf_copy_secret(const char *text, double seconds) {
    @autoreleasepool {
        NSPasteboard *pb = NSPasteboard.generalPasteboard;
        [pb clearContents];
        NSPasteboardItem *item = [NSPasteboardItem new];
        [item setString:@(text) forType:NSPasteboardTypeString];
        [item setString:@"" forType:@"org.nspasteboard.ConcealedType"];
        [item setString:@"" forType:@"org.nspasteboard.TransientType"];
        [pb writeObjects:@[item]];
        NSInteger copied = pb.changeCount;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
          if (NSPasteboard.generalPasteboard.changeCount == copied) {
              [NSPasteboard.generalPasteboard clearContents];
          }
        });
    }
}

// Paths of the files on the general pasteboard, one per line; NULL if none.
char *nf_pasted_files(void) {
    @autoreleasepool {
        NSArray<NSURL *> *urls = [NSPasteboard.generalPasteboard readObjectsForClasses:@[ NSURL.class ]
                                                                               options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}];
        if (!urls.count) return NULL;
        NSMutableArray<NSString *> *paths = [NSMutableArray array];
        for (NSURL *u in urls) [paths addObject:u.path];
        return strdup([paths componentsJoinedByString:@"\n"].UTF8String);
    }
}

// A Finder alias at `alias` to `path`.
char *nf_make_alias(const char *path, const char *alias) {
    @autoreleasepool {
        NSError *error = nil;
        NSData *bookmark = [url(path) bookmarkDataWithOptions:NSURLBookmarkCreationSuitableForBookmarkFile
                               includingResourceValuesForKeys:nil
                                                relativeToURL:nil
                                                        error:&error];
        if (bookmark) [NSURL writeBookmarkData:bookmark toURL:url(alias) options:0 error:&error];
        return message(error);
    }
}

// Apps that open `path`, one path per line (NULL if none): the default,
// then the rest by name, one of each name (several installs of an app
// would otherwise all show).
char *nf_apps_for(const char *path) {
    @autoreleasepool {
        NSWorkspace *ws = NSWorkspace.sharedWorkspace;
        NSString * (^nameOf)(NSURL *) = ^(NSURL *app) { return app.URLByDeletingPathExtension.lastPathComponent; };
        NSURL *file = url(path);
        NSURL *preferred = [ws URLForApplicationToOpenURL:file];
        NSArray<NSURL *> *others = [[ws URLsForApplicationsToOpenURL:file]
            sortedArrayUsingComparator:^NSComparisonResult(NSURL *a, NSURL *b) {
              return [nameOf(a) localizedStandardCompare:nameOf(b)];
            }];
        NSMutableArray<NSString *> *apps = [NSMutableArray array];
        NSMutableSet<NSString *> *names = [NSMutableSet set];
        for (NSURL *app in [@[ preferred ?: NSNull.null ] arrayByAddingObjectsFromArray:others]) {
            if (![app isKindOfClass:NSURL.class]) continue;
            NSString *name = nameOf(app);
            if ([names containsObject:name]) continue;
            [names addObject:name];
            [apps addObject:app.path];
        }
        return apps.count ? strdup([apps componentsJoinedByString:@"\n"].UTF8String) : NULL;
    }
}

void nf_open_with(const char *path, const char *app) {
    @autoreleasepool {
        [NSWorkspace.sharedWorkspace openURLs:@[ url(path) ]
                         withApplicationAtURL:url(app)
                                configuration:[NSWorkspaceOpenConfiguration configuration]
                            completionHandler:nil];
    }
}

// 1 if `path` is where a volume is mounted, | 2 if it can be ejected (disk
// images, removable and network drives).
uint32_t nf_volume_flags(const char *path) {
    @autoreleasepool {
        NSDictionary *values = [url(path) resourceValuesForKeys:@[
            NSURLVolumeURLKey, NSURLVolumeIsEjectableKey, NSURLVolumeIsRemovableKey, NSURLVolumeIsLocalKey
        ] error:nil];
        NSURL *volume = values[NSURLVolumeURLKey];
        if (![volume.path isEqualToString:@(path)]) return 0;
        bool ejectable = [values[NSURLVolumeIsEjectableKey] boolValue] || [values[NSURLVolumeIsRemovableKey] boolValue] ||
                         ![values[NSURLVolumeIsLocalKey] boolValue];
        return 1 | (ejectable ? 2 : 0);
    }
}

// Send files with AirDrop: the system's own AirDrop window, listing who's
// nearby. False if AirDrop can't take them (it's off, say).
bool nf_airdrop(const char *paths) {
    @autoreleasepool {
        NSMutableArray<NSURL *> *urls = [NSMutableArray array];
        for (NSString *p in [@(paths) componentsSeparatedByString:@"\n"]) {
            if (p.length) [urls addObject:url(p.UTF8String)];
        }
        NSSharingService *airdrop = [NSSharingService sharingServiceNamed:NSSharingServiceNameSendViaAirDrop];
        if (!airdrop || ![airdrop canPerformWithItems:urls]) return false;
        [airdrop performWithItems:urls];
        return true;
    }
}

// Unmount and eject the volume at `path`. Blocks until done.
char *nf_eject(const char *path) {
    @autoreleasepool {
        NSError *error = nil;
        [NSWorkspace.sharedWorkspace unmountAndEjectDeviceAtURL:url(path) error:&error];
        return message(error);
    }
}

// One PDF from images, a page each (Quick Actions' Create PDF).
char *nf_images_to_pdf(const char *paths, const char *out) {
    @autoreleasepool {
        PDFDocument *pdf = [PDFDocument new];
        for (NSString *p in [@(paths) componentsSeparatedByString:@"\n"]) {
            // (A PDF: its pages, one after another.)
            if ([p.pathExtension.lowercaseString isEqualToString:@"pdf"]) {
                PDFDocument *doc = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:p]];
                for (NSUInteger i = 0; i < doc.pageCount; i++) {
                    PDFPage *page = [[doc pageAtIndex:i] copy];
                    if (page) [pdf insertPage:page atIndex:pdf.pageCount];
                }
                continue;
            }
            NSImage *image = [[NSImage alloc] initWithContentsOfFile:p];
            PDFPage *page = image ? [[PDFPage alloc] initWithImage:image] : nil;
            if (page) [pdf insertPage:page atIndex:pdf.pageCount];
        }
        if (!pdf.pageCount) return strdup("None of these could be read.");
        return [pdf writeToFile:@(out)] ? NULL : strdup("The PDF couldn't be saved.");
    }
}

// A PDF made smaller: its images saved as JPEG, sized for the screen.
// NULL if done, else why not (malloc'd).
char *nf_compress_pdf(const char *in, const char *out) {
    @autoreleasepool {
        if (@available(macOS 13.4, *)) {
            PDFDocument *doc = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:@(in)]];
            if (!doc) return strdup("That PDF couldn't be read.");
            NSDictionary *options = @{PDFDocumentSaveImagesAsJPEGOption: @YES, PDFDocumentOptimizeImagesForScreenOption: @YES};
            return [doc writeToURL:[NSURL fileURLWithPath:@(out)] withOptions:options] ? NULL : strdup("The smaller PDF couldn't be saved.");
        }
        return strdup("Compressing a PDF needs macOS 13.4 or later.");
    }
}

// Where the app with this bundle identifier is, malloc'd; NULL if it isn't
// installed.
char *nf_app_path(const char *bundle_id) {
    @autoreleasepool {
        NSURL *app = [NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:@(bundle_id)];
        return app ? strdup(app.path.fileSystemRepresentation) : NULL;
    }
}

// A file's SHA-256 (into `out`, 32 bytes), read 1 MB at a time; false if it
// can't be read, or `*stop` turns true on the way.
bool nf_sha256_file(const char *path, uint8_t *out, const volatile bool *stop) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        return false;
    }
    CC_SHA256_CTX ctx;
    CC_SHA256_Init(&ctx);
    static const size_t CHUNK = 1 << 20;
    uint8_t *buf = malloc(CHUNK);
    size_t n;
    bool ok = buf != NULL;
    while (ok && (n = fread(buf, 1, CHUNK, f)) > 0) {
        if (stop && *stop) {
            ok = false;
            break;
        }
        CC_SHA256_Update(&ctx, buf, (CC_LONG)n);
    }
    if (ferror(f)) {
        ok = false;
    }
    free(buf);
    fclose(f);
    if (ok) {
        CC_SHA256_Final(out, &ctx);
    }
    return ok;
}

// What only this file takes on the disk (its blocks no APFS clone of it
// shares), in bytes; -1 if the disk can't say.
int64_t nf_private_size(const char *path) {
    struct attrlist list = {0};
    list.bitmapcount = ATTR_BIT_MAP_COUNT;
    list.forkattr = ATTR_CMNEXT_PRIVATESIZE;
    struct __attribute__((packed)) {
        uint32_t length;
        off_t size;
    } reply;
    if (getattrlist(path, &list, &reply, sizeof(reply), FSOPT_ATTR_CMN_EXTENDED | FSOPT_NOFOLLOW) != 0) {
        return -1;
    }
    return reply.size;
}

// Send a file by AirDrop (`email` false: macOS's picker of who's nearby)
// or in a new email (`email`: the Mail app's, with it attached), as the
// Share menu does. Whether it could be.
bool nf_share_file(const char *path, bool email) {
    NSURL *url = [NSURL fileURLWithPath:@(path)];
    NSSharingService *service = [NSSharingService sharingServiceNamed:email ? NSSharingServiceNameComposeEmail : NSSharingServiceNameSendViaAirDrop];
    if (!service || ![service canPerformWithItems:@[url]]) return false;
    dispatch_async(dispatch_get_main_queue(), ^{
      [NSApp activateIgnoringOtherApps:YES];
      [service performWithItems:@[url]];
    });
    return true;
}

// An image as the desktop picture, on every screen (as Finder's Set Desktop
// Picture does). NULL if done, else why not: malloc'd.
char *nf_set_wallpaper(const char *path) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:@(path)];
        for (NSScreen *screen in NSScreen.screens) {
            NSError *error = nil;
            if (![NSWorkspace.sharedWorkspace setDesktopImageURL:url forScreen:screen options:@{} error:&error]) {
                return strdup((error.localizedDescription ?: @"It couldn't be set.").UTF8String);
            }
        }
        return NULL;
    }
}

// The Mac's region, as two letters ("JP", "IS"): malloc'd, NULL if none.
char *nf_region_code(void) {
    @autoreleasepool {
        NSString *code = [NSLocale.currentLocale objectForKey:NSLocaleCountryCode];
        return code.length ? strdup(code.UTF8String) : NULL;
    }
}

// A property list as JSON can hold it: its data as hex, its dates as
// seconds since 1970.
static id jsonable(id value) {
    if ([value isKindOfClass:NSDictionary.class]) {
        NSMutableDictionary *out = [NSMutableDictionary dictionary];
        [(NSDictionary *)value enumerateKeysAndObjectsUsingBlock:^(id key, id v, __unused BOOL *stop) {
            id j = jsonable(v);
            if (j && [key isKindOfClass:NSString.class]) out[key] = j;
        }];
        return out;
    }
    if ([value isKindOfClass:NSArray.class]) {
        NSMutableArray *out = [NSMutableArray array];
        for (id v in (NSArray *)value) {
            id j = jsonable(v);
            if (j) [out addObject:j];
        }
        return out;
    }
    if ([value isKindOfClass:NSData.class]) {
        NSData *data = value;
        NSMutableString *hex = [NSMutableString stringWithCapacity:data.length * 2];
        const uint8_t *bytes = data.bytes;
        for (NSUInteger i = 0; i < data.length; i++) [hex appendFormat:@"%02x", bytes[i]];
        return hex;
    }
    if ([value isKindOfClass:NSDate.class]) return @([(NSDate *)value timeIntervalSince1970]);
    if ([value isKindOfClass:NSString.class] || [value isKindOfClass:NSNumber.class]) return value;
    return nil;
}

// A property list (binary or XML) as JSON (see `jsonable`): a .DS_Store's
// icon view and window settings, hdiutil's answers. NULL if it isn't one
// (malloc'd).
char *nf_plist_json(const uint8_t *data, size_t len) {
    @autoreleasepool {
        id plist = [NSPropertyListSerialization propertyListWithData:[NSData dataWithBytes:data length:len] options:0 format:NULL error:nil];
        id json = plist ? jsonable(plist) : nil;
        if (![json isKindOfClass:NSDictionary.class] && ![json isKindOfClass:NSArray.class]) return NULL;
        NSData *out = [NSJSONSerialization dataWithJSONObject:json options:0 error:nil];
        return out ? strndup(out.bytes, out.length) : NULL;
    }
}
