#include "audio_capture.h"

#if defined(PLATFORM_UNSUPPORTED)

namespace haven {

class NullAudioCapture : public IAudioCapture {
public:
    bool IsSupported() const override { return false; }
    std::vector<AudioApp> GetAudioApplications() override { return {}; }
    bool StartCapture(uint32_t, CaptureMode, AudioDataCb, CaptureStatusCb) override { return false; }
    void StopCapture() override {}
    void Cleanup() override {}
};

IAudioCapture* CreateAudioCapture() {
    return new NullAudioCapture();
}

} // namespace haven

#endif
