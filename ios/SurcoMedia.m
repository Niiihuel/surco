#import <AVFAudio/AVFAudio.h>
#import <MediaPlayer/MediaPlayer.h>
#import <UIKit/UIKit.h>
#include <stdio.h>

typedef int (*SurcoMediaCallback)(int kind, double value);

static int SurcoMediaResult(BOOL success, NSError *error, char *message, size_t capacity) {
    if (!success && message && capacity) {
        snprintf(message, capacity, "%s", error.localizedDescription.UTF8String ?: "iOS audio session failed");
    }
    return success ? 1 : 0;
}

// https://developer.apple.com/documentation/avfaudio/playing-custom-audio-with-your-own-player
int surco_media_configure(char *message, size_t capacity) {
    @autoreleasepool {
        NSError *error = nil;
        BOOL success = [AVAudioSession.sharedInstance setCategory:AVAudioSessionCategoryPlayback
                                                           mode:AVAudioSessionModeDefault
                                             routeSharingPolicy:AVAudioSessionRouteSharingPolicyLongFormAudio
                                                        options:0
                                                          error:&error];
        return SurcoMediaResult(success, error, message, capacity);
    }
}

int surco_media_activate(char *message, size_t capacity) {
    @autoreleasepool {
        if (!surco_media_configure(message, capacity)) return 0;
        NSError *error = nil;
        BOOL success = [AVAudioSession.sharedInstance setActive:YES error:&error];
        return SurcoMediaResult(success, error, message, capacity);
    }
}

int surco_media_deactivate(char *message, size_t capacity) {
    @autoreleasepool {
        NSError *error = nil;
        BOOL success = [AVAudioSession.sharedInstance setActive:NO
                                                  withOptions:AVAudioSessionSetActiveOptionNotifyOthersOnDeactivation
                                                        error:&error];
        // Apple documents that isBusy still deactivates the session and stops its objects.
        if (!success && error.code == AVAudioSessionErrorCodeIsBusy) {
            NSLog(@"Surco released a running audio session: %@", error);
            success = YES;
        }
        return SurcoMediaResult(success, error, message, capacity);
    }
}

@interface SurcoMedia : NSObject
@property(nonatomic, assign) SurcoMediaCallback callback;
@property(nonatomic, strong) NSMutableArray<id> *observers;
@property(nonatomic, strong) NSMutableArray<NSArray *> *commands;
@property(nonatomic, copy) NSString *trackID;
@property(nonatomic, copy) NSString *artworkURL;
@property(nonatomic, strong) MPMediaItemArtwork *artwork;
@property(nonatomic, copy) NSString *routeSignature;
@end

@implementation SurcoMedia
- (int)send:(int)kind value:(double)value {
    return self.callback ? self.callback(kind, value) : 0;
}

- (NSString *)currentRouteSignature {
    AVAudioSession *session = AVAudioSession.sharedInstance;
    NSMutableArray *ports = [NSMutableArray array];
    for (AVAudioSessionPortDescription *port in session.currentRoute.outputs) [ports addObject:port.UID];
    return [NSString stringWithFormat:@"%@:%g:%ld", ports, session.sampleRate, (long)session.outputNumberOfChannels];
}

- (void)addCommand:(MPRemoteCommand *)command kind:(int)kind {
    __weak SurcoMedia *weakSelf = self;
    id token = [command addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *event) {
        double value = [event isKindOfClass:MPChangePlaybackPositionCommandEvent.class]
            ? ((MPChangePlaybackPositionCommandEvent *)event).positionTime : 0;
        return [weakSelf send:kind value:value] ? MPRemoteCommandHandlerStatusSuccess : MPRemoteCommandHandlerStatusCommandFailed;
    }];
    [self.commands addObject:@[command, token]];
    command.enabled = NO;
}

- (void)start {
    self.observers = [NSMutableArray array];
    self.commands = [NSMutableArray array];
    self.routeSignature = self.currentRouteSignature;
    MPRemoteCommandCenter *center = MPRemoteCommandCenter.sharedCommandCenter;
    [self addCommand:center.playCommand kind:1];
    [self addCommand:center.pauseCommand kind:2];
    [self addCommand:center.togglePlayPauseCommand kind:3];
    [self addCommand:center.nextTrackCommand kind:4];
    [self addCommand:center.previousTrackCommand kind:5];
    [self addCommand:center.changePlaybackPositionCommand kind:6];

    NSNotificationCenter *notifications = NSNotificationCenter.defaultCenter;
    __weak SurcoMedia *weakSelf = self;
    id interrupted = [notifications addObserverForName:AVAudioSessionInterruptionNotification
                                              object:AVAudioSession.sharedInstance queue:NSOperationQueue.mainQueue
                                          usingBlock:^(NSNotification *note) {
        AVAudioSessionInterruptionType type = [note.userInfo[AVAudioSessionInterruptionTypeKey] unsignedIntegerValue];
        if (type == AVAudioSessionInterruptionTypeBegan) {
            [weakSelf send:10 value:0];
        } else if (type == AVAudioSessionInterruptionTypeEnded) {
            NSUInteger options = [note.userInfo[AVAudioSessionInterruptionOptionKey] unsignedIntegerValue];
            [weakSelf send:11 value:(options & AVAudioSessionInterruptionOptionShouldResume) != 0];
        }
    }];
    [self.observers addObject:interrupted];
    id route = [notifications addObserverForName:AVAudioSessionRouteChangeNotification
                                        object:AVAudioSession.sharedInstance queue:NSOperationQueue.mainQueue
                                    usingBlock:^(NSNotification *note) {
        SurcoMedia *owner = weakSelf;
        if (!owner) return;
        NSUInteger reason = [note.userInfo[AVAudioSessionRouteChangeReasonKey] unsignedIntegerValue];
        NSString *signature = owner.currentRouteSignature;
        BOOL changed = ![owner.routeSignature isEqualToString:signature];
        owner.routeSignature = signature;
        if (reason == AVAudioSessionRouteChangeReasonOldDeviceUnavailable) {
            [owner send:12 value:0];
        } else if (changed && (reason == AVAudioSessionRouteChangeReasonNewDeviceAvailable
                           || reason == AVAudioSessionRouteChangeReasonOverride
                           || reason == AVAudioSessionRouteChangeReasonRouteConfigurationChange)) {
            [owner send:13 value:0];
        }
    }];
    [self.observers addObject:route];
    id reset = [notifications addObserverForName:AVAudioSessionMediaServicesWereResetNotification
                                        object:AVAudioSession.sharedInstance queue:NSOperationQueue.mainQueue
                                    usingBlock:^(NSNotification *note) {
        [weakSelf send:14 value:0];
    }];
    [self.observers addObject:reset];
}

- (void)stop {
    self.callback = NULL;
    for (id observer in self.observers) [NSNotificationCenter.defaultCenter removeObserver:observer];
    for (NSArray *entry in self.commands) {
        MPRemoteCommand *command = entry[0];
        [command removeTarget:entry[1]];
        command.enabled = NO;
    }
    [self.observers removeAllObjects];
    [self.commands removeAllObjects];
    MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = nil;
    self.trackID = nil;
    self.artworkURL = nil;
    self.artwork = nil;
}

- (void)publish:(NSDictionary *)snapshot {
    NSDictionary *track = snapshot[@"track"];
    MPRemoteCommandCenter *center = MPRemoteCommandCenter.sharedCommandCenter;
    BOOL hasTrack = [track isKindOfClass:NSDictionary.class];
    for (NSArray *entry in self.commands) ((MPRemoteCommand *)entry[0]).enabled = hasTrack;
    center.changePlaybackPositionCommand.enabled = hasTrack && [snapshot[@"duration"] doubleValue] > 0;
    if (!hasTrack) {
        self.trackID = nil;
        self.artworkURL = nil;
        self.artwork = nil;
        MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = nil;
        return;
    }
    NSString *artworkURL = [track[@"artwork_url"] isKindOfClass:NSString.class] ? track[@"artwork_url"] : nil;
    BOOL sameArt = [self.trackID isEqualToString:track[@"id"]]
        && ((self.artworkURL == nil && artworkURL == nil) || [self.artworkURL isEqualToString:artworkURL]);
    if (!sameArt) self.artwork = nil;
    self.trackID = track[@"id"];
    self.artworkURL = artworkURL;
    NSMutableDictionary *info = [@{
        MPMediaItemPropertyTitle: track[@"title"],
        MPMediaItemPropertyArtist: track[@"artist"],
        MPMediaItemPropertyPlaybackDuration: snapshot[@"duration"],
        MPNowPlayingInfoPropertyElapsedPlaybackTime: snapshot[@"elapsed"],
        MPNowPlayingInfoPropertyPlaybackRate: snapshot[@"rate"],
        MPNowPlayingInfoPropertyDefaultPlaybackRate: @1.0,
        MPNowPlayingInfoPropertyMediaType: @(MPNowPlayingInfoMediaTypeAudio),
        MPNowPlayingInfoPropertyIsLiveStream: @NO,
    } mutableCopy];
    if ([track[@"album"] isKindOfClass:NSString.class]) info[MPMediaItemPropertyAlbumTitle] = track[@"album"];
    if (self.artwork) info[MPMediaItemPropertyArtwork] = self.artwork;
    MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = info;
}
@end

static SurcoMedia *SurcoSystemMedia;

void surco_media_start(SurcoMediaCallback callback) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [SurcoSystemMedia stop];
        SurcoSystemMedia = [SurcoMedia new];
        SurcoSystemMedia.callback = callback;
        [SurcoSystemMedia start];
    });
}

void surco_media_stop(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [SurcoSystemMedia stop];
        SurcoSystemMedia = nil;
    });
}

void surco_media_publish(const char *json) {
    @autoreleasepool {
        NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
        NSDictionary *snapshot = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        if (![snapshot isKindOfClass:NSDictionary.class]) return;
        dispatch_async(dispatch_get_main_queue(), ^{ [SurcoSystemMedia publish:snapshot]; });
    }
}

void surco_media_artwork(const char *track, const char *url, const unsigned char *bytes, size_t length) {
    @autoreleasepool {
        NSString *trackID = [NSString stringWithUTF8String:track];
        NSString *artworkURL = [NSString stringWithUTF8String:url];
        // Copy before returning to Rust; decoding never blocks its realtime audio callback.
        NSData *data = [NSData dataWithBytes:bytes length:length];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            UIImage *image = [UIImage imageWithData:data];
            if (!image) return;
            MPMediaItemArtwork *artwork = [[MPMediaItemArtwork alloc] initWithBoundsSize:image.size
                                                                       requestHandler:^UIImage *(CGSize size) { return image; }];
            dispatch_async(dispatch_get_main_queue(), ^{
                SurcoMedia *owner = SurcoSystemMedia;
                if (![owner.trackID isEqualToString:trackID] || ![owner.artworkURL isEqualToString:artworkURL]) return;
                owner.artwork = artwork;
                NSMutableDictionary *info = [MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo mutableCopy];
                info[MPMediaItemPropertyArtwork] = artwork;
                MPNowPlayingInfoCenter.defaultCenter.nowPlayingInfo = info;
            });
        });
    }
}
