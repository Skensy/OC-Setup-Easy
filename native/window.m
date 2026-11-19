#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>

extern char *oc_request(const char *);
extern void oc_free(char *);

@interface App : NSObject <NSApplicationDelegate, NSWindowDelegate,
                           WKScriptMessageHandlerWithReply,
                           WKNavigationDelegate, WKUIDelegate>
@property(strong) NSWindow *window;
@property(strong) WKWebView *view;
@property(strong) NSURL *page;
@property BOOL dirty;
@property BOOL working;
@property BOOL english;
@end

@implementation App
- (NSString *)it:(NSString *)it en:(NSString *)en {
  return self.english ? en : it;
}
- (void)installMenu {
  NSMenu *menu = [NSMenu new];
  NSMenuItem *main = [menu addItemWithTitle:@"" action:nil keyEquivalent:@""];
  main.submenu = [NSMenu new];
  [main.submenu addItemWithTitle:[self it:@"Esci da OC Setup Easy"
                                       en:@"Quit OC Setup Easy"]
                          action:@selector(terminate:)
                   keyEquivalent:@"q"];
  NSMenuItem *edit = [menu addItemWithTitle:[self it:@"Modifica" en:@"Edit"]
                                     action:nil
                              keyEquivalent:@""];
  edit.submenu = [[NSMenu alloc] initWithTitle:[self it:@"Modifica"
                                                     en:@"Edit"]];
  NSArray *titles = @[
    [self it:@"Annulla" en:@"Undo"], [self it:@"Ripristina" en:@"Redo"],
    [self it:@"Taglia" en:@"Cut"], [self it:@"Copia" en:@"Copy"],
    [self it:@"Incolla" en:@"Paste"],
    [self it:@"Seleziona tutto" en:@"Select All"]
  ];
  NSArray *actions =
      @[ @"undo:", @"redo:", @"cut:", @"copy:", @"paste:", @"selectAll:" ];
  NSArray *keys = @[ @"z", @"Z", @"x", @"c", @"v", @"a" ];
  for (NSUInteger i = 0; i < titles.count; i++)
    [edit.submenu addItemWithTitle:titles[i]
                            action:NSSelectorFromString(actions[i])
                     keyEquivalent:keys[i]];
  NSApp.mainMenu = menu;
}
- (void)applicationDidFinishLaunching:(NSNotification *)note {
  NSString *choice =
      [NSUserDefaults.standardUserDefaults stringForKey:@"InterfaceLanguage"];
  self.english = choice
                     ? [choice isEqual:@"en"]
                     : ![NSLocale.preferredLanguages.firstObject.lowercaseString
                           hasPrefix:@"it"];
  [self installMenu];
  NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
  for (NSNotificationName name in @[
         NSWorkspaceDidMountNotification, NSWorkspaceDidUnmountNotification,
         NSWorkspaceDidRenameVolumeNotification
       ]) {
    [center addObserver:self
               selector:@selector(volumeChanged:)
                   name:name
                 object:nil];
  }
  WKWebViewConfiguration *config = [WKWebViewConfiguration new];
  [config.userContentController
      addScriptMessageHandlerWithReply:self
                          contentWorld:WKContentWorld.pageWorld
                                  name:@"oc"];
  self.view = [[WKWebView alloc] initWithFrame:NSZeroRect configuration:config];
  self.view.navigationDelegate = self;
  self.view.UIDelegate = self;
  self.window = [[NSWindow alloc]
      initWithContentRect:NSMakeRect(0, 0, 1120, 740)
                styleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                           NSWindowStyleMaskMiniaturizable |
                           NSWindowStyleMaskResizable)
                  backing:NSBackingStoreBuffered
                    defer:NO];
  self.window.title = @"OC Setup Easy";
  self.window.minSize = NSMakeSize(800, 540);
  self.window.contentView = self.view;
  self.window.delegate = self;
  self.window.releasedWhenClosed = NO;
  [self.window center];
  [self.window setFrameAutosaveName:@"OCSetupEasyMain"];
  [self.window makeKeyAndOrderFront:nil];
  self.page = [NSBundle.mainBundle URLForResource:@"index"
                                    withExtension:@"html"
                                     subdirectory:@"ui"];
  if (!self.page) {
    fprintf(stderr, "Risorse della finestra assenti. Esegui "
                    "./scripts/build-app.sh e apri il bundle in dist.\n");
    [NSApp terminate:nil];
    return;
  }
  [self.view loadFileURL:self.page
      allowingReadAccessToURL:self.page.URLByDeletingLastPathComponent];
  [NSApp activateIgnoringOtherApps:YES];
}
- (void)volumeChanged:(NSNotification *)notification {
  dispatch_async(dispatch_get_main_queue(), ^{
    [self.view evaluateJavaScript:
                   @"window.dispatchEvent(new Event('ocVolumesChanged'))"
                completionHandler:nil];
  });
}
- (void)webView:(WKWebView *)web
    decidePolicyForNavigationAction:(WKNavigationAction *)action
                    decisionHandler:(void (^)(WKNavigationActionPolicy))decide {
  decide([action.request.URL isEqual:self.page] && action.targetFrame.mainFrame
             ? WKNavigationActionPolicyAllow
             : WKNavigationActionPolicyCancel);
}
- (BOOL)confirm:(NSString *)text button:(NSString *)button {
  NSAlert *alert = [NSAlert new];
  alert.messageText = text;
  [alert addButtonWithTitle:[self it:@"Annulla" en:@"Cancel"]];
  [alert addButtonWithTitle:button];
  return [alert runModal] == NSAlertSecondButtonReturn;
}
- (void)webView:(WKWebView *)web
    runJavaScriptConfirmPanelWithMessage:(NSString *)message
                        initiatedByFrame:(WKFrameInfo *)frame
                       completionHandler:(void (^)(BOOL))done {
  done([self confirm:message button:[self it:@"Continua" en:@"Continue"]]);
}
- (BOOL)canClose {
  if (self.working) {
    NSBeep();
    return NO;
  }
  return !self.dirty ||
         [self confirm:[self it:@"Scartare le modifiche non salvate?"
                             en:@"Discard unsaved changes?"]
                button:[self it:@"Scarta" en:@"Discard"]];
}
- (BOOL)windowShouldClose:(NSWindow *)sender {
  if (![self canClose])
    return NO;
  self.dirty = NO;
  return YES;
}
- (NSApplicationTerminateReply)applicationShouldTerminate:
    (NSApplication *)sender {
  return [self canClose] ? NSTerminateNow : NSTerminateCancel;
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:
    (NSApplication *)sender {
  return YES;
}
- (void)userContentController:(WKUserContentController *)controller
      didReceiveScriptMessage:(WKScriptMessage *)msg
                 replyHandler:(void (^)(id, NSString *))reply {
  if (!msg.frameInfo.mainFrame ||
      ![msg.frameInfo.request.URL isEqual:self.page] ||
      ![msg.body isKindOfClass:NSDictionary.class]) {
    reply(nil, [self it:@"Origine non valida" en:@"Invalid origin"]);
    return;
  }
  NSMutableDictionary *req = [msg.body mutableCopy];
  NSString *op = req[@"op"];
  if ([op isEqual:@"language"]) {
    NSString *choice = req[@"value"];
    if ([choice isEqual:@"it"] || [choice isEqual:@"en"]) {
      self.english = [choice isEqual:@"en"];
      [NSUserDefaults.standardUserDefaults setObject:choice
                                              forKey:@"InterfaceLanguage"];
      [self installMenu];
    }
    reply(@{@"ok" : self.english ? @"en" : @"it"}, nil);
    return;
  }
  if ([op isEqual:@"dirty"]) {
    self.dirty = [req[@"value"] boolValue];
    reply(@{}, nil);
    return;
  }
  if (self.working) {
    reply(nil, [self it:@"Operazione in corso" en:@"Operation in progress"]);
    return;
  }
  if ([op isEqual:@"export"]) {
    NSSavePanel *panel = [NSSavePanel savePanel];
    panel.title = [self it:@"Esporta config anonimizzato"
                        en:@"Export anonymized config"];
    panel.prompt = [self it:@"Esporta" en:@"Export"];
    panel.nameFieldStringValue = @"config-anonymous.plist";
    if ([panel runModal] != NSModalResponseOK) {
      reply(@{@"cancelled" : @YES}, nil);
      return;
    }
    req[@"path"] = panel.URL.path;
  }
  if (([op isEqual:@"open"] && !req[@"volume"]) || [op isEqual:@"import"]) {
    BOOL importing = [op isEqual:@"import"];
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.title = importing ? [self it:@"Scegli il kext da copiare"
                                    en:@"Choose a kext to import"]
                            : [self it:@"Apri il volume o la cartella EFI / OC"
                                    en:@"Open an EFI volume or folder"];
    panel.prompt = [self it:@"Apri" en:@"Open"];
    panel.canChooseDirectories = YES;
    panel.canChooseFiles = importing;
    panel.allowsMultipleSelection = NO;
    if ([panel runModal] != NSModalResponseOK) {
      reply(@{@"cancelled" : @YES}, nil);
      return;
    }
    req[@"path"] = panel.URL.path;
  } else if ([op isEqual:@"open"]) {
    req[@"path"] = req[@"volume"];
  }
  NSData *data = [NSJSONSerialization dataWithJSONObject:req
                                                 options:0
                                                   error:nil];
  NSString *json = [[NSString alloc] initWithData:data
                                         encoding:NSUTF8StringEncoding];
  self.working = YES;
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    char *raw = oc_request(json.UTF8String);
    NSData *response = [NSData dataWithBytes:raw length:strlen(raw)];
    oc_free(raw);
    NSDictionary *result = [NSJSONSerialization JSONObjectWithData:response
                                                           options:0
                                                             error:nil];
    dispatch_async(dispatch_get_main_queue(), ^{
      self.working = NO;
      if (result)
        reply(result, nil);
      else
        reply(nil, [self it:@"Risposta non valida" en:@"Invalid response"]);
    });
  });
}
@end

void run_window(void) {
  @autoreleasepool {
    NSApplication *app = [NSApplication sharedApplication];
    [app setActivationPolicy:NSApplicationActivationPolicyRegular];
    App *delegate = [App new];
    app.delegate = delegate;
    [app run];
    (void)delegate.window;
  }
}
