// Public MediaPlayer APIs only. No event taps, global media-key hooks or private frameworks.
#import <AppKit/AppKit.h>
#import <MediaPlayer/MediaPlayer.h>
#import <ImageIO/ImageIO.h>
#import <dispatch/dispatch.h>
#include <atomic>
#include <cmath>
#include "macos_media.h"

static void OnMain(dispatch_block_t block) {
    if ([NSThread isMainThread]) block();
    else dispatch_sync(dispatch_get_main_queue(), block);
}
static NSString *StringValue(id value) { return [value isKindOfClass:NSString.class] ? value : @""; }

@interface LSNowPlaying : NSObject {
    LiushengMediaCallback _callback;
    NSMutableArray<NSArray *> *_targets;
    NSDictionary *_snapshot;
    NSMutableDictionary *_info;
    MPMediaItemArtwork *_artwork;
    NSString *_artURL;
    id _terminationObserver;
    dispatch_queue_t _artQueue;
    std::atomic<uint64_t> _artGeneration;
    BOOL _registered;
    BOOL _claimed;
    NSTimeInterval _timeAnchor;
}
- (instancetype)initWithCallback:(LiushengMediaCallback)callback;
- (BOOL)publish:(NSDictionary *)snapshot;
- (MPRemoteCommandHandlerStatus)route:(int32_t)command value:(double)value;
- (void)stop;
- (void)addCommand:(MPRemoteCommand *)command code:(int32_t)code;
- (void)updateCommands;
- (void)writeInfo;
- (void)loadArtwork:(NSString *)urlText;
#ifdef LIUSHENG_MEDIA_TEST
- (NSUInteger)targetCount;
- (BOOL)claimed;
#endif
@end

@implementation LSNowPlaying
- (instancetype)initWithCallback:(LiushengMediaCallback)callback {
    self = [super init];
    if (!self) return nil;
    _callback = callback;
    _registered = YES;
    _claimed = NO;
    _targets = [NSMutableArray array];
    _snapshot = @{};
    _artGeneration.store(0);
    _artQueue = dispatch_queue_create("io.github.dhkun.Liusheng.artwork", DISPATCH_QUEUE_SERIAL);
    MPRemoteCommandCenter *center = MPRemoteCommandCenter.sharedCommandCenter;
    [self addCommand:center.playCommand code:1];
    [self addCommand:center.pauseCommand code:2];
    [self addCommand:center.togglePlayPauseCommand code:3];
    [self addCommand:center.nextTrackCommand code:4];
    [self addCommand:center.previousTrackCommand code:5];
    [self addCommand:center.stopCommand code:6];
    __weak LSNowPlaying *weakSelf = self;
    id position = [center.changePlaybackPositionCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
        LSNowPlaying *owner = weakSelf;
        if (!owner || ![event isKindOfClass:MPChangePlaybackPositionCommandEvent.class]) return MPRemoteCommandHandlerStatusCommandFailed;
        return [owner route:7 value:((MPChangePlaybackPositionCommandEvent *)event).positionTime];
    }];
    [_targets addObject:@[center.changePlaybackPositionCommand, position]];
    center.skipForwardCommand.preferredIntervals = @[@15];
    center.skipBackwardCommand.preferredIntervals = @[@15];
    for (MPSkipIntervalCommand *command in @[center.skipForwardCommand, center.skipBackwardCommand]) {
        const double sign = command == center.skipForwardCommand ? 1.0 : -1.0;
        id token = [command addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
            LSNowPlaying *owner = weakSelf;
            if (!owner || ![event isKindOfClass:MPSkipIntervalCommandEvent.class]) return MPRemoteCommandHandlerStatusCommandFailed;
            const double interval = ((MPSkipIntervalCommandEvent *)event).interval;
            if (!std::isfinite(interval) || interval < 0) return MPRemoteCommandHandlerStatusCommandFailed;
            return [owner route:8 value:sign * interval];
        }];
        [_targets addObject:@[command, token]];
    }
    id repeat = [center.changeRepeatModeCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
        LSNowPlaying *owner = weakSelf;
        if (!owner || ![event isKindOfClass:MPChangeRepeatModeCommandEvent.class]) return MPRemoteCommandHandlerStatusCommandFailed;
        const MPRepeatType mode = ((MPChangeRepeatModeCommandEvent *)event).repeatType;
        if (mode != MPRepeatTypeOff && mode != MPRepeatTypeOne && mode != MPRepeatTypeAll) return MPRemoteCommandHandlerStatusCommandFailed;
        return [owner route:9 value:mode == MPRepeatTypeOne ? 1 : mode == MPRepeatTypeAll ? 2 : 0];
    }];
    [_targets addObject:@[center.changeRepeatModeCommand, repeat]];
    id shuffle = [center.changeShuffleModeCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
        LSNowPlaying *owner = weakSelf;
        if (!owner || ![event isKindOfClass:MPChangeShuffleModeCommandEvent.class]) return MPRemoteCommandHandlerStatusCommandFailed;
        const MPShuffleType mode = ((MPChangeShuffleModeCommandEvent *)event).shuffleType;
        if (mode != MPShuffleTypeOff && mode != MPShuffleTypeItems) return MPRemoteCommandHandlerStatusCommandFailed;
        return [owner route:10 value:mode == MPShuffleTypeItems ? 1 : 0];
    }];
    [_targets addObject:@[center.changeShuffleModeCommand, shuffle]];
    [self updateCommands];
    _terminationObserver = [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationWillTerminateNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *notification) {
        (void)notification;
        [weakSelf stop];
    }];
    return self;
}
- (void)addCommand:(MPRemoteCommand *)command code:(int32_t)code {
    __weak LSNowPlaying *weakSelf = self;
    id token = [command addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
        (void)event;
        LSNowPlaying *owner = weakSelf;
        return owner ? [owner route:code value:0] : MPRemoteCommandHandlerStatusCommandFailed;
    }];
    [_targets addObject:@[command, token]];
}
- (void)updateCommands {
    MPRemoteCommandCenter *c = MPRemoteCommandCenter.sharedCommandCenter;
    const BOOL available = _registered && _claimed && [_snapshot[@"hasTrack"] boolValue];
    c.playCommand.enabled = available;
    c.pauseCommand.enabled = available;
    c.togglePlayPauseCommand.enabled = available;
    c.stopCommand.enabled = available;
    c.previousTrackCommand.enabled = available && [_snapshot[@"canPrevious"] boolValue];
    c.nextTrackCommand.enabled = available && [_snapshot[@"canNext"] boolValue];
    c.changePlaybackPositionCommand.enabled = available && [_snapshot[@"canSeek"] boolValue];
    c.skipForwardCommand.enabled = c.changePlaybackPositionCommand.enabled;
    c.skipBackwardCommand.enabled = c.changePlaybackPositionCommand.enabled;
    c.changeRepeatModeCommand.enabled = available;
    c.changeShuffleModeCommand.enabled = available;
    c.changeRepeatModeCommand.currentRepeatType = [_snapshot[@"repeat"] intValue] == 1 ? MPRepeatTypeOne : [_snapshot[@"repeat"] intValue] == 2 ? MPRepeatTypeAll : MPRepeatTypeOff;
    c.changeShuffleModeCommand.currentShuffleType = [_snapshot[@"shuffle"] boolValue] ? MPShuffleTypeItems : MPShuffleTypeOff;
}
- (MPRemoteCommandHandlerStatus)route:(int32_t)command value:(double)value {
    __block MPRemoteCommandHandlerStatus result = MPRemoteCommandHandlerStatusCommandFailed;
    OnMain(^{
        if (!self->_registered || !self->_claimed || ![self->_snapshot[@"hasTrack"] boolValue]) return;
        if (command < 1 || command > 10 || !std::isfinite(value)) return;
        if (command == 4 && ![self->_snapshot[@"canNext"] boolValue]) return;
        if (command == 5 && ![self->_snapshot[@"canPrevious"] boolValue]) return;
        if (command == 7 || command == 8) {
            if (![self->_snapshot[@"canSeek"] boolValue]) return;
            if (command == 7 && (value < 0 || value > [self->_snapshot[@"duration"] doubleValue])) return;
        }
        if (command == 9 && value != 0 && value != 1 && value != 2) return;
        if (command == 10 && value != 0 && value != 1) return;
        if (self->_callback && self->_callback(command, value)) result = MPRemoteCommandHandlerStatusSuccess;
    });
    return result;
}
- (void)writeInfo {
    // macOS requires BOTH playbackState and the elapsed-time/rate dictionary.
    MPNowPlayingInfoCenter *center = MPNowPlayingInfoCenter.defaultCenter;
    if (_info) {
        const double advance = [_snapshot[@"status"] intValue] == 1 ? std::fmax(0.0, NSProcessInfo.processInfo.systemUptime - _timeAnchor) : 0.0;
        const double position = [_snapshot[@"position"] doubleValue] + advance;
        const double duration = [_snapshot[@"duration"] doubleValue];
        _info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = @(duration > 0 ? std::fmin(duration, position) : position);
    }
    center.nowPlayingInfo = _info;
    center.playbackState = [_snapshot[@"status"] intValue] == 1 ? MPNowPlayingPlaybackStatePlaying : MPNowPlayingPlaybackStatePaused;
}
- (void)loadArtwork:(NSString *)urlText {
    if ([_artURL isEqualToString:urlText]) return;
    _artURL = [urlText copy];
    _artwork = nil;
    const uint64_t generation = _artGeneration.fetch_add(1) + 1;
    NSURL *url = [NSURL URLWithString:urlText];
    if (!url.isFileURL) return;
    __weak LSNowPlaying *weakSelf = self;
    dispatch_async(_artQueue, ^{
        @autoreleasepool {
            LSNowPlaying *owner = weakSelf;
            if (!owner || owner->_artGeneration.load() != generation) return;
            NSNumber *fileSize = nil;
            if (![url getResourceValue:&fileSize forKey:NSURLFileSizeKey error:nullptr] || fileSize.unsignedLongLongValue > 16 * 1024 * 1024) return;
            CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)url, nullptr);
            if (!source) return;
            NSDictionary *options = @{(__bridge NSString *)kCGImageSourceCreateThumbnailFromImageAlways:@YES,
                                      (__bridge NSString *)kCGImageSourceCreateThumbnailWithTransform:@YES,
                                      (__bridge NSString *)kCGImageSourceThumbnailMaxPixelSize:@768};
            CGImageRef thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)options);
            CFRelease(source);
            if (!thumbnail) return;
            dispatch_async(dispatch_get_main_queue(), ^{
                if (owner->_registered && owner->_claimed && owner->_artGeneration.load() == generation) {
                    NSImage *image = [[NSImage alloc] initWithCGImage:thumbnail size:NSMakeSize(CGImageGetWidth(thumbnail), CGImageGetHeight(thumbnail))];
                    MPMediaItemArtwork *art = [[MPMediaItemArtwork alloc] initWithBoundsSize:image.size requestHandler:^NSImage *(CGSize requestedSize) {
                        (void)requestedSize;
                        return image;
                    }];
                    owner->_artwork = art;
                    owner->_info[MPMediaItemPropertyArtwork] = art;
                    [owner writeInfo];
                }
                CGImageRelease(thumbnail);
            });
        }
    });
}
- (BOOL)publish:(NSDictionary *)snapshot {
    if (!_registered) return NO;
    const int status = [snapshot[@"status"] intValue];
    const double duration = [snapshot[@"duration"] doubleValue];
    const double position = [snapshot[@"position"] doubleValue];
    if (status < 0 || status > 2 || !std::isfinite(duration) || !std::isfinite(position) || duration < 0 || position < 0) return NO;
    _snapshot = [snapshot copy];
    _timeAnchor = NSProcessInfo.processInfo.systemUptime;
    if (![snapshot[@"hasTrack"] boolValue] || status == 0) {
        _claimed = NO;
        _artGeneration.fetch_add(1);
        _artURL = nil;
        _artwork = nil;
        _info = nil;
        MPNowPlayingInfoCenter.defaultCenter.playbackState = MPNowPlayingPlaybackStateStopped;
        MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = nil;
        [self updateCommands];
        return YES;
    }
    // Restoring a paused queue at app startup must not claim another app's media keys.
    if (status == 1) _claimed = YES;
    [self updateCommands];
    if (!_claimed) return YES;
    [self loadArtwork:StringValue(snapshot[@"artUrl"])];
    _info = [@{MPMediaItemPropertyTitle:StringValue(snapshot[@"title"]),
               MPMediaItemPropertyArtist:StringValue(snapshot[@"artist"]),
               MPMediaItemPropertyAlbumTitle:StringValue(snapshot[@"album"]),
               MPMediaItemPropertyPlaybackDuration:@(duration),
               MPNowPlayingInfoPropertyElapsedPlaybackTime:@(duration > 0 ? std::fmin(position, duration) : position),
               MPNowPlayingInfoPropertyPlaybackRate:status == 1 ? @1.0 : @0.0,
               MPNowPlayingInfoPropertyDefaultPlaybackRate:@1.0,
               MPNowPlayingInfoPropertyIsLiveStream:@NO,
               MPNowPlayingInfoPropertyMediaType:@(MPNowPlayingInfoMediaTypeAudio),
               MPNowPlayingInfoPropertyExternalContentIdentifier:StringValue(snapshot[@"id"]),
               MPNowPlayingInfoPropertyPlaybackQueueIndex:snapshot[@"queueIndex"] ?: @0,
               MPNowPlayingInfoPropertyPlaybackQueueCount:snapshot[@"queueLength"] ?: @0} mutableCopy];
    if (_artwork) _info[MPMediaItemPropertyArtwork] = _artwork;
    [self writeInfo];
    return YES;
}
- (void)stop {
    if (!_registered) return;
    _registered = NO;
    _claimed = NO;
    _callback = nullptr;
    _artGeneration.fetch_add(1);
    for (NSArray *pair in _targets) {
        MPRemoteCommand *command = pair[0];
        [command removeTarget:pair[1]];
        command.enabled = NO;
    }
    [_targets removeAllObjects];
    if (_terminationObserver) {
        [NSNotificationCenter.defaultCenter removeObserver:_terminationObserver];
        _terminationObserver = nil;
    }
    _info = nil;
    _artwork = nil;
    MPNowPlayingInfoCenter.defaultCenter.playbackState = MPNowPlayingPlaybackStateStopped;
    MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = nil;
}
#ifdef LIUSHENG_MEDIA_TEST
- (NSUInteger)targetCount { return _targets.count; }
- (BOOL)claimed { return _claimed; }
#endif
@end

static LSNowPlaying *service;
extern "C" bool liusheng_media_start(LiushengMediaCallback callback) {
    if (!callback) return false;
    __block bool success = false;
    OnMain(^{
        @try {
            if (service) return;
            service = [[LSNowPlaying alloc] initWithCallback:callback];
            success = service != nil;
        } @catch (NSException *exception) { NSLog(@"Liusheng MediaPlayer registration: %@", exception.reason); }
    });
    return success;
}
extern "C" bool liusheng_media_publish(const uint8_t *bytes, size_t length) {
    if (!bytes || length == 0 || length > 1024 * 1024) return false;
    // Copy synchronously: Rust owns the original byte slice.
    NSData *data = [NSData dataWithBytes:bytes length:length];
    __block bool success = false;
    OnMain(^{
        @autoreleasepool {
            @try {
                id json = [NSJSONSerialization JSONObjectWithData:data options:0 error:nullptr];
                if ([json isKindOfClass:NSDictionary.class] && service) success = [service publish:json];
            } @catch (NSException *exception) { NSLog(@"Liusheng Now Playing update: %@", exception.reason); }
        }
    });
    return success;
}
extern "C" void liusheng_media_stop(void) {
    OnMain(^{
        @try { [service stop]; }
        @catch (NSException *exception) { NSLog(@"Liusheng MediaPlayer cleanup: %@", exception.reason); }
        @finally { service = nil; }
    });
}
