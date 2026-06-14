#pragma once

#include "agbot_flight_sim/RayTracedCamera.hpp"
#include "agbot_flight_sim/Vec3.hpp"

#include <cstddef>
#include <string>
#include <vector>

namespace agbot::flight_sim {

enum class StreamDeliveryStatus {
    Delivered,
    DeliveryFailed,
    TimestampMisaligned,
};

struct StreamTelemetrySample {
    std::string drone_id;
    double timestamp_s = 0.0;
    Vec3 position;
    std::string mode;
};

struct EncodedVideoFrame {
    std::size_t frame_index = 0;
    double timestamp_s = 0.0;
    std::string frame_hash;
    std::string codec = "agbot-ray-frame-v1";
    std::string payload;
};

struct TelemetryReceipt {
    StreamTelemetrySample sample;
    bool acked = false;
};

struct VideoReceipt {
    EncodedVideoFrame frame;
    bool acked = false;
    bool decoded = false;
};

struct StreamConfig {
    double max_timestamp_skew_s = 0.05;
    bool buffer_on_failure = true;
};

class LocalTelemetryVideoCollector {
public:
    void set_available(bool available);
    [[nodiscard]] bool available() const;
    [[nodiscard]] bool collect_telemetry(const StreamTelemetrySample& sample);
    [[nodiscard]] bool collect_video(const EncodedVideoFrame& frame);
    [[nodiscard]] const std::vector<TelemetryReceipt>& telemetry_receipts() const;
    [[nodiscard]] const std::vector<VideoReceipt>& video_receipts() const;

private:
    bool available_ = true;
    std::vector<TelemetryReceipt> telemetry_receipts_;
    std::vector<VideoReceipt> video_receipts_;
};

struct StreamDeliveryReport {
    StreamDeliveryStatus status = StreamDeliveryStatus::DeliveryFailed;
    std::size_t sent_telemetry_count = 0;
    std::size_t sent_video_count = 0;
    std::size_t acked_telemetry_count = 0;
    std::size_t acked_video_count = 0;
    std::size_t decoded_video_count = 0;
    std::size_t buffered_telemetry_count = 0;
    std::size_t buffered_video_count = 0;
    bool timestamp_alignment_ok = false;
    std::string delivery_failure_reason;
    std::vector<EncodedVideoFrame> encoded_video;

    [[nodiscard]] std::string to_json() const;
};

[[nodiscard]] const char* to_string(StreamDeliveryStatus status);
[[nodiscard]] EncodedVideoFrame encode_raytraced_frame(
    const RayTracedFrame& frame,
    std::size_t frame_index);
[[nodiscard]] bool decode_encoded_video_frame(const EncodedVideoFrame& frame);

[[nodiscard]] StreamDeliveryReport stream_telemetry_and_video(
    const std::vector<StreamTelemetrySample>& telemetry,
    const std::vector<RayTracedFrame>& frames,
    LocalTelemetryVideoCollector& collector,
    StreamConfig config = {});

} // namespace agbot::flight_sim
