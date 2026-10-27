// The login keychain, for what F2 keeps secret: a password item of its
// own (service and account), readable by F2 without asking (it made it).

#import <Foundation/Foundation.h>
#import <Security/Security.h>

static NSDictionary *nf_keychain_query(const char *service, const char *account) {
    return @{
        (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService: @(service),
        (__bridge id)kSecAttrAccount: @(account),
    };
}

// `secret` kept as the item's password (replacing it, if there's one).
// Whether that worked.
bool nf_keychain_set(const char *service, const char *account, const char *secret) {
    @autoreleasepool {
        NSDictionary *query = nf_keychain_query(service, account);
        NSData *data = [@(secret) dataUsingEncoding:NSUTF8StringEncoding];
        OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)@{(__bridge id)kSecValueData: data});
        if (status == errSecItemNotFound) {
            NSMutableDictionary *item = [query mutableCopy];
            item[(__bridge id)kSecValueData] = data;
            item[(__bridge id)kSecAttrLabel] = @(service);
            status = SecItemAdd((__bridge CFDictionaryRef)item, NULL);
        }
        return status == errSecSuccess;
    }
}

// The item's password (malloc'd), or NULL: there's none.
char *nf_keychain_get(const char *service, const char *account) {
    @autoreleasepool {
        NSMutableDictionary *query = [nf_keychain_query(service, account) mutableCopy];
        query[(__bridge id)kSecReturnData] = @YES;
        query[(__bridge id)kSecMatchLimit] = (__bridge id)kSecMatchLimitOne;
        CFTypeRef found = NULL;
        if (SecItemCopyMatching((__bridge CFDictionaryRef)query, &found) != errSecSuccess || !found) {
            return NULL;
        }
        NSData *data = (__bridge_transfer NSData *)found;
        NSString *secret = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return secret ? strdup(secret.UTF8String) : NULL;
    }
}

// The item gone (if it was there).
void nf_keychain_delete(const char *service, const char *account) {
    @autoreleasepool {
        SecItemDelete((__bridge CFDictionaryRef)nf_keychain_query(service, account));
    }
}
