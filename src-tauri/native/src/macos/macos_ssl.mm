#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>
#import <Security/Security.h>
#import <ifaddrs.h>
#import <net/if.h>
#import <netinet/in.h>
#include <vector>

typedef void (^ChallengeCompletionHandler)(NSURLSessionAuthChallengeDisposition disposition, NSURLCredential * _Nullable credential);
typedef void (*HavenTrustDecide)(void* webview, const char* host, const uint8_t* const* ders, const size_t* lens, size_t count, const char* page, int loading, uint64_t request);
typedef void (*HavenTrustDone)(const char* key, int yes);

static void (*g_original_challenge_impl)(id, SEL, WKWebView*, NSURLAuthenticationChallenge*, ChallengeCompletionHandler) = NULL;
static HavenTrustDecide g_decide = NULL;
static uint64_t g_next_request = 1;
static NSMutableDictionary<NSNumber*, NSArray*>* g_waiting = nil;

// Called on the main thread once the system has rejected the certificate.
// Rust decides (pins, one question per host and key) and answers through
// haven_macos_trust_finish, now or after the user has replied.
static void ask_rust(WKWebView* webView, NSURLAuthenticationChallenge* challenge, ChallengeCompletionHandler completionHandler) {
    SecTrustRef trust = challenge.protectionSpace.serverTrust;
    NSMutableArray<NSData*>* certs = [NSMutableArray array];
    // SecTrustCopyCertificateChain needs macOS 12; the app supports 10.15.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    for (CFIndex i = 0; i < SecTrustGetCertificateCount(trust); i++) {
        [certs addObject:(__bridge_transfer NSData*)SecCertificateCopyData(SecTrustGetCertificateAtIndex(trust, i))];
    }
#pragma clang diagnostic pop
    uint64_t request = g_next_request++;
    if (!g_waiting) g_waiting = [NSMutableDictionary dictionary];
    g_waiting[@(request)] = @[[completionHandler copy], challenge];
    std::vector<const uint8_t*> ptrs;
    std::vector<size_t> lens;
    for (NSData* d in certs) {
        ptrs.push_back((const uint8_t*)d.bytes);
        lens.push_back(d.length);
    }
    const char* page = webView.URL.absoluteString.UTF8String;
    g_decide((__bridge void*)webView, challenge.protectionSpace.host.UTF8String ?: "", ptrs.data(), lens.data(), ptrs.size(), page ?: "", webView.isLoading ? 1 : 0, request);
}

static void custom_challenge_handler(id self, SEL _cmd, WKWebView* webView, NSURLAuthenticationChallenge* challenge, ChallengeCompletionHandler completionHandler) {
    if ([challenge.protectionSpace.authenticationMethod isEqualToString:NSURLAuthenticationMethodServerTrust] && challenge.protectionSpace.serverTrust && g_decide) {
        // The system's own verdict first (keychain roots, user and company CAs,
        // missing intermediates fetched by the system). It can touch the
        // network, so it runs off the main thread.
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            bool trusted = SecTrustEvaluateWithError(challenge.protectionSpace.serverTrust, NULL);
            dispatch_async(dispatch_get_main_queue(), ^{
                if (trusted) {
                    completionHandler(NSURLSessionAuthChallengePerformDefaultHandling, nil);
                } else {
                    ask_rust(webView, challenge, completionHandler);
                }
            });
        });
        return;
    }
    if (g_original_challenge_impl) {
        g_original_challenge_impl(self, _cmd, webView, challenge, completionHandler);
    } else {
        completionHandler(NSURLSessionAuthChallengePerformDefaultHandling, nil);
    }
}

extern "C" void haven_macos_trust_attach(void* webview_ptr, HavenTrustDecide decide) {
    if (!webview_ptr || !decide) return;
    g_decide = decide;
    @autoreleasepool {
        WKWebView* webView = (__bridge WKWebView*)webview_ptr;
        id delegate = webView.navigationDelegate;
        if (!delegate) return;
        Class cls = [delegate class];
        SEL challengeSel = @selector(webView:didReceiveAuthenticationChallenge:completionHandler:);
        Method method = class_getInstanceMethod(cls, challengeSel);
        if (method) {
            IMP oldImp = method_getImplementation(method);
            if (oldImp != (IMP)custom_challenge_handler) {
                g_original_challenge_impl = (void(*)(id, SEL, WKWebView*, NSURLAuthenticationChallenge*, ChallengeCompletionHandler))oldImp;
                method_setImplementation(method, (IMP)custom_challenge_handler);
            }
        } else {
            const char* typeEncoding = "v@:@@@?";
            class_addMethod(cls, challengeSel, (IMP)custom_challenge_handler, typeEncoding);
        }
    }
}

extern "C" void haven_macos_trust_finish(uint64_t request, int allow) {
    NSArray* entry = g_waiting[@(request)];
    if (!entry) return;
    [g_waiting removeObjectForKey:@(request)];
    ChallengeCompletionHandler completionHandler = entry[0];
    NSURLAuthenticationChallenge* challenge = entry[1];
    if (allow) {
        completionHandler(NSURLSessionAuthChallengeUseCredential, [NSURLCredential credentialForTrust:challenge.protectionSpace.serverTrust]);
    } else {
        completionHandler(NSURLSessionAuthChallengeCancelAuthenticationChallenge, nil);
    }
}

@interface HavenDetailsToggle : NSObject
@property (weak) NSAlert* alert;
@property (strong) NSView* box;
@property (strong) NSTextField* details;
@property CGFloat closedHeight;
@property CGFloat openHeight;
- (void)toggle:(NSButton*)sender;
@end

@implementation HavenDetailsToggle
- (void)toggle:(NSButton*)sender {
    BOOL open = sender.state == NSControlStateValueOn;
    CGFloat width = self.box.frame.size.width;
    CGFloat height = open ? self.openHeight : self.closedHeight;
    self.details.hidden = !open;
    [self.box setFrameSize:NSMakeSize(width, height)];
    for (NSView* v in self.box.subviews) {
        if (v != self.details) [v setFrameOrigin:NSMakePoint(v.frame.origin.x, height - self.closedHeight + (self.closedHeight - v.frame.size.height) / 2)];
    }
    [self.alert layout];
}
@end

// One certificate question, as a sheet on the window that hit it.
// Cancel is the default button; Details folds out the issuer, dates and
// SHA-256 fingerprint.
extern "C" void haven_macos_trust_ask(void* webview_ptr, const char* key, const char* title, const char* heading, const char* body, const char* details, const char* more, const char* trust, const char* cancel, HavenTrustDone done) {
    NSString* answerKey = @(key);
    NSAlert* alert = [[NSAlert alloc] init];
    alert.alertStyle = NSAlertStyleWarning;
    alert.messageText = @(heading);
    alert.informativeText = @(body);
    alert.window.title = @(title);
    [alert addButtonWithTitle:@(cancel)];
    [alert addButtonWithTitle:@(trust)].keyEquivalent = @"";

    CGFloat width = 320;
    NSTextField* info = [NSTextField wrappingLabelWithString:@(details)];
    info.selectable = YES;
    info.font = [NSFont systemFontOfSize:[NSFont smallSystemFontSize]];
    info.preferredMaxLayoutWidth = width;
    NSSize infoSize = [info.cell cellSizeForBounds:NSMakeRect(0, 0, width, CGFLOAT_MAX)];
    info.frame = NSMakeRect(0, 0, width, ceil(infoSize.height));
    info.hidden = YES;

    NSButton* disclose = [[NSButton alloc] initWithFrame:NSMakeRect(0, 0, 13, 13)];
    disclose.bezelStyle = NSBezelStyleDisclosure;
    [disclose setButtonType:NSButtonTypePushOnPushOff];
    disclose.title = @"";
    NSTextField* label = [NSTextField labelWithString:@(more)];
    [label sizeToFit];
    CGFloat row = MAX(label.frame.size.height, 18);

    HavenDetailsToggle* toggle = [[HavenDetailsToggle alloc] init];
    toggle.closedHeight = row;
    toggle.openHeight = row + 6 + info.frame.size.height;
    NSView* box = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, width, row)];
    disclose.frame = NSMakeRect(0, (row - 13) / 2, 13, 13);
    label.frame = NSMakeRect(18, (row - label.frame.size.height) / 2, label.frame.size.width, label.frame.size.height);
    [box addSubview:disclose];
    [box addSubview:label];
    [box addSubview:info];
    toggle.box = box;
    toggle.details = info;
    disclose.target = toggle;
    disclose.action = @selector(toggle:);
    alert.accessoryView = box;
    toggle.alert = alert;
    objc_setAssociatedObject(alert, "havenDetailsToggle", toggle, OBJC_ASSOCIATION_RETAIN_NONATOMIC);

    void (^finish)(NSModalResponse) = ^(NSModalResponse response) {
        done(answerKey.UTF8String, response == NSAlertSecondButtonReturn ? 1 : 0);
    };
    NSWindow* window = webview_ptr ? ((__bridge WKWebView*)webview_ptr).window : nil;
    if (window && window.isVisible) {
        [alert beginSheetModalForWindow:window completionHandler:finish];
    } else {
        dispatch_async(dispatch_get_main_queue(), ^{
            finish([alert runModal]);
        });
    }
}

// Interfaces that are up and carry an IPv4 address. macOS keeps several
// utun interfaces (iCloud Private Relay, Continuity) with IPv6 link-local
// addresses only, so an IPv4 address is what marks a VPN tunnel.
extern "C" void haven_macos_ipv4_interfaces(void (*found)(const char* name, void* user), void* user) {
    struct ifaddrs* list = NULL;
    if (!found || getifaddrs(&list) != 0) return;
    for (struct ifaddrs* a = list; a; a = a->ifa_next) {
        if (a->ifa_addr && a->ifa_addr->sa_family == AF_INET && (a->ifa_flags & IFF_UP) && (a->ifa_flags & IFF_RUNNING)) {
            found(a->ifa_name, user);
        }
    }
    freeifaddrs(list);
}

extern "C" void haven_macos_get_clipboard_files(void (*callback)(const char* utf8_path, void* user), void* user) {
    if (!callback) return;
    @autoreleasepool {
        NSPasteboard* pboard = [NSPasteboard generalPasteboard];
        NSArray<Class> *classes = @[[NSURL class]];
        NSDictionary *options = @{NSPasteboardURLReadingFileURLsOnlyKey: @YES};
        NSArray *urls = [pboard readObjectsForClasses:classes options:options];
        for (NSURL *url in urls) {
            if ([url isFileURL]) {
                const char* path = [[url path] UTF8String];
                if (path) {
                    callback(path, user);
                }
            }
        }
    }
}

extern "C" void haven_macos_set_badge_label(const char* label) {
    @autoreleasepool {
        NSDockTile *tile = [[NSApplication sharedApplication] dockTile];
        if (label && strlen(label) > 0) {
            [tile setBadgeLabel:[NSString stringWithUTF8String:label]];
        } else {
            [tile setBadgeLabel:nil];
        }
    }
}
