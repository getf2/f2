// Contacts, for Quick Open's "call mom", "message sara", "email dad":
// people found by name, with their phone numbers and email addresses
// (macOS asks, the first time).

#import <Contacts/Contacts.h>
#import <Foundation/Foundation.h>

// Whether F2 may look at Contacts: 1 yes, 0 not asked yet, -1 no.
int nf_contacts_access(void) {
    switch ([CNContactStore authorizationStatusForEntityType:CNEntityTypeContacts]) {
        case CNAuthorizationStatusAuthorized: return 1;
        case CNAuthorizationStatusNotDetermined: return 0;
        default: return -1;
    }
}

// Ask for Contacts (macOS asks, the first time), waiting for the answer:
// not on the main thread. Whether it's allowed.
bool nf_contacts_request(void) {
    @autoreleasepool {
        __block BOOL granted = NO;
        dispatch_semaphore_t answered = dispatch_semaphore_create(0);
        [[CNContactStore new] requestAccessForEntityType:CNEntityTypeContacts
                                       completionHandler:^(BOOL ok, NSError *__unused error) {
                                         granted = ok;
                                         dispatch_semaphore_signal(answered);
                                       }];
        dispatch_semaphore_wait(answered, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC));
        return granted;
    }
}

static NSString *clean(NSString *s) {
    return [[s stringByReplacingOccurrencesOfString:@"\t" withString:@" "] stringByReplacingOccurrencesOfString:@"\n" withString:@" "];
}

// People named like `name` (by first, last or nickname), at most `limit`:
// a line each of their numbers and addresses, "name \t phone|email \t
// label \t value" (malloc'd; NULL if Contacts can't be looked at).
char *nf_contacts_find(const char *name, int limit) {
    @autoreleasepool {
        if (nf_contacts_access() != 1) return NULL;
        NSArray *keys = @[
            [CNContactFormatter descriptorForRequiredKeysForStyle:CNContactFormatterStyleFullName], CNContactNicknameKey, CNContactOrganizationNameKey,
            CNContactPhoneNumbersKey, CNContactEmailAddressesKey
        ];
        NSArray<CNContact *> *people = [[CNContactStore new] unifiedContactsMatchingPredicate:[CNContact predicateForContactsMatchingName:@(name)] keysToFetch:keys error:nil];
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        int n = 0;
        for (CNContact *person in people) {
            if (n++ >= limit) break;
            NSString *called = [CNContactFormatter stringFromContact:person style:CNContactFormatterStyleFullName];
            if (!called.length) called = person.nickname.length ? person.nickname : person.organizationName;
            if (!called.length) continue;
            for (CNLabeledValue<CNPhoneNumber *> *phone in person.phoneNumbers) {
                NSString *label = phone.label ? [CNLabeledValue localizedStringForLabel:phone.label] : @"";
                [lines addObject:[NSString stringWithFormat:@"%@\tphone\t%@\t%@", clean(called), clean(label), clean(phone.value.stringValue)]];
            }
            for (CNLabeledValue<NSString *> *email in person.emailAddresses) {
                NSString *label = email.label ? [CNLabeledValue localizedStringForLabel:email.label] : @"";
                [lines addObject:[NSString stringWithFormat:@"%@\temail\t%@\t%@", clean(called), clean(label), clean(email.value)]];
            }
        }
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}
