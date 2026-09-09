// Compile the production adapter into this isolated AppKit test executable.
// No hardware events or private MediaPlayer APIs are synthesized.
#define LIUSHENG_MEDIA_TEST 1
#include "../../crates/liusheng/src/macos_media.mm"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <utility>

static std::vector<std::pair<int32_t, double>> received;
static bool acceptCommands = true;
static bool Receive(int32_t code, double value) { received.emplace_back(code, value); return acceptCommands; }
#define REQUIRE(condition) do { if (!(condition)) { std::fprintf(stderr, "MEDIA FAIL line %d: %s\n", __LINE__, #condition); liusheng_media_stop(); return 1; } } while (0)
static bool Publish(NSDictionary *snapshot) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:snapshot options:0 error:nullptr];
    return liusheng_media_publish((const uint8_t *)data.bytes, data.length);
}
static void Pump(double seconds) {
    const NSTimeInterval until = NSProcessInfo.processInfo.systemUptime + seconds;
    while (NSProcessInfo.processInfo.systemUptime < until)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
}
int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        REQUIRE(!liusheng_media_start(nullptr));
        REQUIRE(liusheng_media_start(Receive));
        REQUIRE(service.targetCount == 11);
        REQUIRE(!liusheng_media_start(Receive));
        REQUIRE(!MPRemoteCommandCenter.sharedCommandCenter.playCommand.enabled);
        NSMutableDictionary *state = [@{@"hasTrack":@YES, @"status":@2, @"title":@"测试 / Test / 音楽", @"artist":@"Artist", @"album":@"Album", @"id":@"opaque-track-1", @"artUrl":@"", @"duration":@120.0, @"position":@30.0, @"queueIndex":@0, @"queueLength":@2, @"canSeek":@YES, @"canNext":@YES, @"repeat":@0, @"shuffle":@NO} mutableCopy];
        REQUIRE(Publish(state));
        REQUIRE(!service.claimed);
        REQUIRE([service route:1 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        state[@"status"] = @1;
        REQUIRE(Publish(state));
        REQUIRE(service.claimed);
        MPNowPlayingInfoCenter *center = MPNowPlayingInfoCenter.defaultCenter;
        NSDictionary *info = center.nowPlayingInfo;
        REQUIRE([info[MPMediaItemPropertyTitle] isEqualToString:state[@"title"]]);
        REQUIRE([info[MPMediaItemPropertyArtist] isEqualToString:@"Artist"]);
        REQUIRE([info[MPMediaItemPropertyAlbumTitle] isEqualToString:@"Album"]);
        REQUIRE([info[MPMediaItemPropertyPlaybackDuration] doubleValue] == 120.0);
        REQUIRE([info[MPNowPlayingInfoPropertyPlaybackRate] doubleValue] == 1.0);
        REQUIRE(center.playbackState == MPNowPlayingPlaybackStatePlaying);
        for (int code = 1; code <= 6; code++) REQUIRE([service route:code value:0] == MPRemoteCommandHandlerStatusSuccess);
        REQUIRE([service route:7 value:42.5] == MPRemoteCommandHandlerStatusSuccess);
        REQUIRE(received.back().first == 7 && received.back().second == 42.5);
        REQUIRE([service route:8 value:-15.0] == MPRemoteCommandHandlerStatusSuccess);
        REQUIRE([service route:9 value:2.0] == MPRemoteCommandHandlerStatusSuccess);
        REQUIRE([service route:10 value:1.0] == MPRemoteCommandHandlerStatusSuccess);
        REQUIRE([service route:7 value:-1.0] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE([service route:7 value:121.0] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE([service route:7 value:NAN] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE([service route:99 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        acceptCommands = false;
        REQUIRE([service route:1 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        acceptCommands = true;
        state[@"canSeek"] = @NO; state[@"canNext"] = @NO;
        REQUIRE(Publish(state));
        REQUIRE(!MPRemoteCommandCenter.sharedCommandCenter.changePlaybackPositionCommand.enabled);
        REQUIRE([service route:4 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE([service route:7 value:40] == MPRemoteCommandHandlerStatusCommandFailed);
        state[@"status"] = @2;
        state[@"position"] = @42.5;
        state[@"repeat"] = @2; state[@"shuffle"] = @YES;
        REQUIRE(Publish(state));
        REQUIRE(center.playbackState == MPNowPlayingPlaybackStatePaused);
        REQUIRE([center.nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] doubleValue] == 0);
        REQUIRE([center.nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] doubleValue] == 42.5);
        REQUIRE(MPRemoteCommandCenter.sharedCommandCenter.changeRepeatModeCommand.currentRepeatType == MPRepeatTypeAll);
        REQUIRE(MPRemoteCommandCenter.sharedCommandCenter.changeShuffleModeCommand.currentShuffleType == MPShuffleTypeItems);

        // File-URL artwork loads off the main thread; rapid replacement cannot
        // resurrect another song's cover, including after service shutdown.
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:2 pixelsHigh:2 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:8 bitsPerPixel:32];
        memset(bitmap.bitmapData, 127, 16);
        NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:[NSUUID.UUID.UUIDString stringByAppendingString:@" cover.png"]];
        REQUIRE([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:path atomically:YES]);
        state[@"artUrl"] = [NSURL fileURLWithPath:path].absoluteString;
        REQUIRE(Publish(state));
        const NSTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 5;
        while (!center.nowPlayingInfo[MPMediaItemPropertyArtwork] && NSProcessInfo.processInfo.systemUptime < deadline) Pump(0.02);
        REQUIRE(center.nowPlayingInfo[MPMediaItemPropertyArtwork] != nil);
        REQUIRE([center.nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] doubleValue] == 42.5);
        state[@"id"] = @"opaque-track-2";
        state[@"artUrl"] = @"https://invalid.example/never-download.png";
        REQUIRE(Publish(state));
        REQUIRE(center.nowPlayingInfo[MPMediaItemPropertyArtwork] == nil);
        state[@"artUrl"] = [NSURL fileURLWithPath:path].absoluteString;
        REQUIRE(Publish(state));
        state[@"hasTrack"] = @NO; state[@"status"] = @0;
        REQUIRE(Publish(state));
        Pump(0.1);
        REQUIRE(center.nowPlayingInfo == nil);
        REQUIRE(!MPRemoteCommandCenter.sharedCommandCenter.playCommand.enabled);
        REQUIRE([service route:3 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE(!liusheng_media_publish((const uint8_t *)"bad json", 8));
        LSNowPlaying *old = service;
        liusheng_media_stop();
        REQUIRE(old.targetCount == 0);
        REQUIRE([old route:1 value:0] == MPRemoteCommandHandlerStatusCommandFailed);
        REQUIRE(liusheng_media_start(Receive));
        REQUIRE(service.targetCount == 11);
        liusheng_media_stop();
        [NSFileManager.defaultManager removeItemAtPath:path error:nullptr];
        std::puts("Native MediaPlayer: metadata, state, 11 registrations, commands, bounds, artwork races and cleanup passed");
    }
    return 0;
}
