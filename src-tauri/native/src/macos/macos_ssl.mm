#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>
#import <Security/Security.h>

typedef void (^ChallengeCompletionHandler)(NSURLSessionAuthChallengeDisposition disposition, NSURLCredential * _Nullable credential);

static void (*g_original_challenge_impl)(id, SEL, WKWebView*, NSURLAuthenticationChallenge*, ChallengeCompletionHandler) = NULL;

static void custom_challenge_handler(id self, SEL _cmd, WKWebView* webView, NSURLAuthenticationChallenge* challenge, ChallengeCompletionHandler completionHandler) {
    if ([challenge.protectionSpace.authenticationMethod isEqualToString:NSURLAuthenticationMethodServerTrust]) {
        SecTrustRef trust = challenge.protectionSpace.serverTrust;
        if (trust) {
            NSURLCredential* cred = [NSURLCredential credentialForTrust:trust];
            completionHandler(NSURLSessionAuthChallengeUseCredential, cred);
            return;
        }
    }
    if (g_original_challenge_impl) {
        g_original_challenge_impl(self, _cmd, webView, challenge, completionHandler);
    } else {
        completionHandler(NSURLSessionAuthChallengePerformDefaultHandling, nil);
    }
}

extern "C" void haven_macos_accept_self_signed(void* webview_ptr) {
    if (!webview_ptr) return;
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
