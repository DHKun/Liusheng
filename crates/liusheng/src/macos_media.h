#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif
// Stable transport shared with macos_media.rs. Values use seconds at this ABI.
typedef bool (*LiushengMediaCallback)(int32_t command, double value);
bool liusheng_media_start(LiushengMediaCallback callback);
bool liusheng_media_publish(const uint8_t *bytes, size_t length);
void liusheng_media_stop(void);
#ifdef __cplusplus
}
#endif
