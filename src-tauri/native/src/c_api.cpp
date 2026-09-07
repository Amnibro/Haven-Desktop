#include "haven_audio_c.h"
#include "audio_capture.h"

#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>

using namespace haven;

static std::unique_ptr<IAudioCapture> g_capture;
static std::mutex g_mu;
static HavenAudioDataCb g_data_cb = nullptr;
static HavenAudioStatusCb g_status_cb = nullptr;
static void* g_user = nullptr;

static IAudioCapture* Cap() {
    if (!g_capture) g_capture.reset(CreateAudioCapture());
    return g_capture.get();
}

extern "C" {

int haven_audio_is_supported(void) {
    try { return Cap()->IsSupported() ? 1 : 0; }
    catch (...) { return 0; }
}

int haven_audio_get_apps(HavenAudioAppC** out_apps, size_t* out_count) {
    if (!out_apps || !out_count) return 0;
    *out_apps = nullptr;
    *out_count = 0;
    try {
        auto apps = Cap()->GetAudioApplications();
        if (apps.empty()) return 1;
        auto* arr = static_cast<HavenAudioAppC*>(std::calloc(apps.size(), sizeof(HavenAudioAppC)));
        if (!arr) return 0;
        for (size_t i = 0; i < apps.size(); i++) {
            arr[i].pid = apps[i].pid;
            arr[i].name = ::strdup(apps[i].name.c_str());
            arr[i].icon = ::strdup(apps[i].icon.c_str());
            arr[i].active = apps[i].active ? 1 : 0;
        }
        *out_apps = arr;
        *out_count = apps.size();
        return 1;
    } catch (...) {
        return 0;
    }
}

void haven_audio_free_apps(HavenAudioAppC* apps, size_t count) {
    if (!apps) return;
    for (size_t i = 0; i < count; i++) {
        std::free(apps[i].name);
        std::free(apps[i].icon);
    }
    std::free(apps);
}

int haven_audio_start_capture(uint32_t pid, const char* mode,
                              HavenAudioDataCb data_cb,
                              HavenAudioStatusCb status_cb,
                              void* user) {
    CaptureMode m = CaptureMode::IncludeProcess;
    if (mode && std::string(mode) == "exclude") m = CaptureMode::ExcludeProcess;

    {
        std::lock_guard<std::mutex> lock(g_mu);
        g_data_cb = data_cb;
        g_status_cb = status_cb;
        g_user = user;
    }

    AudioDataCb dataWrap = [](const float* data, size_t count) {
        HavenAudioDataCb cb = nullptr;
        void* u = nullptr;
        {
            std::lock_guard<std::mutex> lock(g_mu);
            cb = g_data_cb;
            u = g_user;
        }
        if (cb) cb(data, count, u);
    };

    CaptureStatusCb statusWrap = [](const CaptureStatus& s) {
        HavenAudioStatusCb cb = nullptr;
        void* u = nullptr;
        {
            std::lock_guard<std::mutex> lock(g_mu);
            cb = g_status_cb;
            u = g_user;
        }
        if (!cb) return;
        const char* kind = "stopped";
        switch (s.kind) {
            case CaptureStatusKind::Starting: kind = "starting"; break;
            case CaptureStatusKind::Started:  kind = "started"; break;
            case CaptureStatusKind::Failed:   kind = "failed"; break;
            case CaptureStatusKind::Stopped:  kind = "stopped"; break;
        }
        cb(kind, s.message.c_str(), s.code, u);
    };

    try {
        return Cap()->StartCapture(pid, m, std::move(dataWrap), std::move(statusWrap)) ? 1 : 0;
    } catch (...) {
        return 0;
    }
}

void haven_audio_stop_capture(void) {
    try { Cap()->StopCapture(); } catch (...) {}
    std::lock_guard<std::mutex> lock(g_mu);
    g_data_cb = nullptr;
    g_status_cb = nullptr;
    g_user = nullptr;
}

void haven_audio_cleanup(void) {
    try {
        if (g_capture) {
            g_capture->StopCapture();
            g_capture->Cleanup();
            g_capture.reset();
        }
    } catch (...) {}
    std::lock_guard<std::mutex> lock(g_mu);
    g_data_cb = nullptr;
    g_status_cb = nullptr;
    g_user = nullptr;
}

} // extern "C"
