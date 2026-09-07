#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct HavenAudioAppC {
    uint32_t pid;
    char* name;
    char* icon;
    int active;
} HavenAudioAppC;

typedef void (*HavenAudioDataCb)(const float* data, size_t count, void* user);
typedef void (*HavenAudioStatusCb)(const char* kind, const char* message, int64_t code, void* user);

int  haven_audio_is_supported(void);
int  haven_audio_get_apps(HavenAudioAppC** out_apps, size_t* out_count);
void haven_audio_free_apps(HavenAudioAppC* apps, size_t count);
int  haven_audio_start_capture(uint32_t pid, const char* mode,
                               HavenAudioDataCb data_cb,
                               HavenAudioStatusCb status_cb,
                               void* user);
void haven_audio_stop_capture(void);
void haven_audio_cleanup(void);

#ifdef __cplusplus
}
#endif
