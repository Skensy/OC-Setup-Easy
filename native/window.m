#import <Cocoa/Cocoa.h>
#import <CoreServices/CoreServices.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#import <WebKit/WebKit.h>

extern char *oc_request(const char *);
extern void oc_free(char *);

BOOL oc_english(void) {
  NSString *choice = [NSUserDefaults.standardUserDefaults
      persistentDomainForName:@"io.github.skensy.ocsetupeasy"]
      [@"InterfaceLanguage"];
  return choice ? [choice isEqual:@"en"]
                : ![NSLocale.preferredLanguages.firstObject.lowercaseString
                      hasPrefix:@"it"];
}

@interface App : NSObject <NSApplicationDelegate, NSWindowDelegate,
                           WKScriptMessageHandlerWithReply,
                           WKNavigationDelegate, WKUIDelegate>
@property(strong) NSWindow *window;
@property(strong) WKWebView *view;
@property(strong) NSURL *page;
@property BOOL dirty;
@property BOOL closing;
@property BOOL editingTask;
@property NSUInteger pending;
@property(strong) dispatch_queue_t requests;
@property FSEventStreamRef stream;
@property(copy) NSString *root;
- (void)changedPaths:(NSArray *)paths
               flags:(const FSEventStreamEventFlags *)flags;

@property BOOL english;
@end

static void fileEvents(ConstFSEventStreamRef stream, void *context,
                       size_t count, void *paths,
                       const FSEventStreamEventFlags flags[],
                       const FSEventStreamEventId ids[]) {
  (void)count;
  (void)ids;
  if (stream != [(__bridge App *)context stream])
    return;
  [(__bridge App *)context changedPaths:(__bridge NSArray *)paths flags:flags];
}

@implementation App
- (BOOL)watch:(NSString *)root {
  if (_stream) {
    FSEventStreamStop(_stream);
    FSEventStreamInvalidate(_stream);
    FSEventStreamRelease(_stream);
    _stream = NULL;
  }
  self.root = root;
  self.window.representedFilename =
      root ? [root stringByAppendingPathComponent:@"config.plist"] : @"";
  if (!root)
    return YES;
  FSEventStreamContext context = {0, (__bridge void *)self, NULL, NULL, NULL};
  _stream = FSEventStreamCreate(
      NULL, fileEvents, &context, (__bridge CFArrayRef) @[ root ],
      kFSEventStreamEventIdSinceNow, 0.25,
      kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagFileEvents |
          kFSEventStreamCreateFlagWatchRoot);
  if (_stream) {
    FSEventStreamSetDispatchQueue(_stream, dispatch_get_main_queue());
    if (FSEventStreamStart(_stream))
      return YES;
    FSEventStreamInvalidate(_stream);
    FSEventStreamRelease(_stream);
    _stream = NULL;
  }
  return NO;
}
- (void)changedPaths:(NSArray *)paths
               flags:(const FSEventStreamEventFlags *)flags {
  BOOL config = NO, folders = NO;
  NSString *prefix = [self.root stringByAppendingString:@"/"];
  for (NSUInteger i = 0; i < paths.count; i++) {
    if (flags[i] & (kFSEventStreamEventFlagMustScanSubDirs |
                    kFSEventStreamEventFlagRootChanged)) {
      folders = config = YES;
    }
    NSString *path = paths[i];
    if (![path hasPrefix:prefix])
      continue;
    NSString *first = [[[path substringFromIndex:prefix.length]
                           pathComponents].firstObject lowercaseString];
    if ([first isEqual:@"config.plist"])
      config = YES;
    if ([@[ @"acpi", @"kexts", @"drivers", @"tools" ] containsObject:first])
      folders = YES;
  }
  if (config || folders)
    [self.view callAsyncJavaScript:@"window.filesChanged(root, config, folders)"
                         arguments:@{
                           @"root" : self.root,
                           @"config" : @(config),
                           @"folders" : @(folders)
                         }
                           inFrame:nil
                    inContentWorld:WKContentWorld.pageWorld
                 completionHandler:nil];
}
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
  for (NSUInteger i = 0; i < titles.count; i++) {
    NSMenuItem *item =
        [edit.submenu addItemWithTitle:titles[i]
                                action:i < 2 ? @selector(editHistory:)
                                             : NSSelectorFromString(actions[i])
                         keyEquivalent:keys[i]];
    if (i < 2) {
      item.target = self;
      item.tag = i;
    }
  }
  NSMenuItem *view = [menu addItemWithTitle:[self it:@"Vista" en:@"View"] action:nil keyEquivalent:@""];
  view.submenu = [[NSMenu alloc] initWithTitle:view.title];
  NSArray *views = @[[self it:@"Panoramica" en:@"Overview"], [self it:@"Volumi EFI" en:@"EFI volumes"],
    [self it:@"Editor plist" en:@"Plist editor"], [self it:@"Kext e componenti" en:@"Kexts and components"],
    [self it:@"Verifica e backup" en:@"Validation and backups"], [self it:@"Hardware e sistema" en:@"Hardware and system"],
    [self it:@"Cerca" en:@"Find"], [self it:@"Monta o smonta EFI" en:@"Mount or unmount EFI"]];
  NSArray *shortcuts = @[@"1",@"2",@"3",@"4",@"5",@"6",@"f",@"e"];
  for (NSUInteger i = 0; i < views.count; ++i) {
    NSMenuItem *item = [view.submenu addItemWithTitle:views[i] action:@selector(viewShortcut:) keyEquivalent:shortcuts[i]];
    item.target = self;
  }
  NSApp.mainMenu = menu;
}
- (void)viewShortcut:(NSMenuItem *)item {
  [self.view callAsyncJavaScript:@"window.ocShortcut(key)" arguments:@{@"key":item.keyEquivalent}
    inFrame:nil inContentWorld:WKContentWorld.pageWorld completionHandler:nil];
}
- (void)editHistory:(NSMenuItem *)item {
  [self.view
      callAsyncJavaScript:@"if "
                          @"(document.activeElement.matches('input,textarea,["
                          @"contenteditable=true]')) return true; if (st.w) "
                          @"await work(() => act(op)); return false"
                arguments:@{@"op" : item.tag ? @"redo" : @"undo"}
                  inFrame:nil
           inContentWorld:WKContentWorld.pageWorld
        completionHandler:^(id native, NSError *error) {
          if (!error && [native isEqual:@YES])
            [NSApp sendAction:item.tag ? @selector(redo:) : @selector(undo:)
                           to:nil
                         from:item];
        }];
}
- (void)applicationDidFinishLaunching:(NSNotification *)note {
  self.english = oc_english();
  self.requests =
      dispatch_queue_create("io.github.skensy.requests", DISPATCH_QUEUE_SERIAL);
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
  NSVisualEffectView *material = [[NSVisualEffectView alloc] initWithFrame:self.window.contentView.bounds];
  material.material = NSVisualEffectMaterialSidebar;
  material.blendingMode = NSVisualEffectBlendingModeBehindWindow;
  material.state = NSVisualEffectStateFollowsWindowActiveState;
  [self.view setValue:@NO forKey:@"drawsBackground"];
  self.view.frame = material.bounds;
  self.view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  [material addSubview:self.view];
  self.window.contentView = material;
  self.window.toolbarStyle = NSWindowToolbarStyleUnified;
  self.window.titlebarAppearsTransparent = YES;
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
- (NSApplicationTerminateReply)requestClose:(BOOL)quit {
  if (self.closing || self.pending || self.editingTask) {
    NSBeep();
    return NSTerminateCancel;
  }
  if (!self.dirty)
    return NSTerminateNow;
  self.closing = YES;
  NSAlert *alert = [NSAlert new];
  alert.messageText = [self it:@"Salvare le modifiche al config.plist?"
                            en:@"Save changes to config.plist?"];
  [alert addButtonWithTitle:[self it:@"Salva" en:@"Save"]];
  NSButton *discard = [alert addButtonWithTitle:[self it:@"Scarta"
                                                      en:@"Discard"]];
  discard.hasDestructiveAction = YES;
  [alert addButtonWithTitle:[self it:@"Annulla" en:@"Cancel"]].keyEquivalent =
      @"\e";
  NSModalResponse answer = [alert runModal];
  if (answer != NSAlertFirstButtonReturn) {
    self.closing = NO;
    if (answer == NSAlertSecondButtonReturn)
      self.dirty = NO;
    return self.dirty ? NSTerminateCancel : NSTerminateNow;
  }
  [self.view
      callAsyncJavaScript:@"await work(() => act('save')); return !st.dirty"
                arguments:@{}
                  inFrame:nil
           inContentWorld:WKContentWorld.pageWorld
        completionHandler:^(id saved, NSError *error) {
          self.closing = NO;
          BOOL ok = !error && [saved isEqual:@YES] && !self.dirty;
          if (quit)
            [NSApp replyToApplicationShouldTerminate:ok];
          else if (ok)
            [self.window performClose:nil];
        }];
  return NSTerminateLater;
}
- (BOOL)windowShouldClose:(NSWindow *)sender {
  return [self requestClose:NO] == NSTerminateNow;
}
- (NSApplicationTerminateReply)applicationShouldTerminate:
    (NSApplication *)sender {
  return [self requestClose:YES];
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
  if ([op isEqual:@"editingTask"]) {
    self.editingTask = [req[@"value"] boolValue];
    reply(@{}, nil);
    return;
  }
  if ([op isEqual:@"dirty"]) {
    self.dirty = [req[@"value"] boolValue];
    self.window.documentEdited = self.dirty;
    reply(@{}, nil);
    return;
  }
  if ([op isEqual:@"sample"])
    [req removeObjectForKey:@"path"];
  if ([op isEqual:@"sample"] && [req[@"choose"] boolValue]) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.title = [self it:@"Seleziona Sample.plist" en:@"Choose Sample.plist"];
    panel.allowedContentTypes = @[ UTTypePropertyList ];
    panel.canChooseDirectories = NO;
    if ([panel runModal] != NSModalResponseOK) {
      reply(@{@"cancelled" : @YES}, nil);
      return;
    }
    req[@"path"] = panel.URL.path;
  }
  if ([op isEqual:@"compile"]) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.title = [self it:@"Compila sorgente ACPI" en:@"Compile ACPI source"];
    panel.allowedContentTypes = @[[UTType typeWithFilenameExtension:@"dsl"]];
    panel.canChooseDirectories = NO;
    if ([panel runModal] != NSModalResponseOK) { reply(@{@"cancelled":@YES}, nil); return; }
    req[@"path"] = panel.URL.path;
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
  self.pending++;
  dispatch_async(self.requests, ^{
    char *raw = oc_request(json.UTF8String);
    NSData *response = [NSData dataWithBytes:raw length:strlen(raw)];
    oc_free(raw);
    NSDictionary *result = [NSJSONSerialization JSONObjectWithData:response
                                                           options:0
                                                             error:nil];
    dispatch_async(dispatch_get_main_queue(), ^{
      self.pending--;
      if (result[@"ok"] && [op isEqual:@"open"]) {
        NSMutableDictionary *opened = [result[@"ok"] mutableCopy];
        opened[@"watching"] = @([self watch:opened[@"root"]]);
        reply(@{@"ok" : opened}, nil);
        return;
      }
      if (result[@"ok"] && ([op isEqual:@"close"] || [op isEqual:@"finish"]))
        [self watch:nil];
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
