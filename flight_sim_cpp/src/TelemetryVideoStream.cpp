#include "agbot_flight_sim/TelemetryVideoStream.hpp"

#include "agbot_flight_sim/TwinContractV1.hpp"

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <string_view>

namespace agbot::flight_sim {
namespace {

std::string escape_json(std::string_view value) {
    std::ostringstream output;
    for (const char c : value) {
        switch (c) {
            case '"':
                output << "\\\"";
                break;
            case '\\':
                output << "\\\\";
                break;
            case '\n':
                output << "\\n";
                break;
            case '\r':
                output << "\\r";
                break;
            case '\t':
                output << "\\t";
                break;
            default:
                output << c;
                break;
        }
    }
    return output.str();
}

void write_double(std::ostringstream& output, double value, int precision = 3) {
    output << std::fixed << std::setprecision(precision) << value;
}

bool timestamp_aligned(
    const std::vector<StreamTelemetrySample>& telemetry,
    const std::vector<RayTracedFrame>& frames,
    double max_skew_s) {
    if (!std::isfinite(max_skew_s) || max_skew_s < 0.0 || telemetry.empty() || frames.empty()) {
        return false;
    }
    return std::all_of(frames.begin(), frames.end(), [&](const RayTracedFrame& frame) {
        return std::any_of(telemetry.begin(), telemetry.end(), [&](const StreamTelemetrySample& sample) {
            return std::abs(sample.timestamp_s - frame.timestamp_s) <= max_skew_s;
        });
    });
}

StreamDeliveryReport failed_report(
    const std::vector<StreamTelemetrySample>& telemetry,
    const std::vector<EncodedVideoFrame>& encoded_video,
    std::string reason,
    bool buffer_on_failure) {
    StreamDeliveryReport report;
    report.status = StreamDeliveryStatus::DeliveryFailed;
    report.delivery_failure_reason = std::move(reason);
    report.encoded_video = encoded_video;
    if (buffer_on_failure) {
        report.buffered_telemetry_count = telemetry.size();
        report.buffered_video_count = encoded_video.size();
    }
    return report;
}

} // namespace

const char* to_string(StreamDeliveryStatus status) {
    switch (status) {
        case StreamDeliveryStatus::Delivered:
            return "delivered";
        case StreamDeliveryStatus::DeliveryFailed:
            return "delivery_failed";
        case StreamDeliveryStatus::TimestampMisaligned:
            return "timestamp_misaligned";
    }
    return "unknown";
}

void LocalTelemetryVideoCollector::set_available(bool available) {
    available_ = available;
}

bool LocalTelemetryVideoCollector::available() const {
    return available_;
}

bool LocalTelemetryVideoCollector::collect_telemetry(const StreamTelemetrySample& sample) {
    if (!available_) {
        return false;
    }
    telemetry_receipts_.push_back({sample, true});
    return true;
}

bool LocalTelemetryVideoCollector::collect_video(const EncodedVideoFrame& frame) {
    if (!available_) {
        return false;
    }
    video_receipts_.push_back({frame, true, decode_encoded_video_frame(frame)});
    return true;
}

const std::vector<TelemetryReceipt>& LocalTelemetryVideoCollector::telemetry_receipts() const {
    return telemetry_receipts_;
}

const std::vector<VideoReceipt>& LocalTelemetryVideoCollector::video_receipts() const {
    return video_receipts_;
}

EncodedVideoFrame encode_raytraced_frame(const RayTracedFrame& frame, std::size_t frame_index) {
    EncodedVideoFrame encoded;
    encoded.frame_index = frame_index;
    encoded.timestamp_s = frame.timestamp_s;
    encoded.frame_hash = frame.frame_hash;
    encoded.payload = "AGBOT-RAY-FRAME-V1|"
        + frame.frame_hash
        + "|"
        + sha256_hex(frame.to_json());
    return encoded;
}

bool decode_encoded_video_frame(const EncodedVideoFrame& frame) {
    const std::string prefix = "AGBOT-RAY-FRAME-V1|" + frame.frame_hash + "|";
    return frame.codec == "agbot-ray-frame-v1"
        && !frame.frame_hash.empty()
        && frame.payload.rfind(prefix, 0) == 0
        && frame.payload.size() > prefix.size();
}

std::string StreamDeliveryReport::to_json() const {
    std::ostringstream output;
    output << "{"
           << "\"status\":\"" << to_string(status) << "\""
           << ",\"sent_telemetry_count\":" << sent_telemetry_count
           << ",\"sent_video_count\":" << sent_video_count
           << ",\"acked_telemetry_count\":" << acked_telemetry_count
           << ",\"acked_video_count\":" << acked_video_count
           << ",\"decoded_video_count\":" << decoded_video_count
           << ",\"buffered_telemetry_count\":" << buffered_telemetry_count
           << ",\"buffered_video_count\":" << buffered_video_count
           << ",\"timestamp_alignment_ok\":" << (timestamp_alignment_ok ? "true" : "false")
           << ",\"delivery_failure_reason\":\"" << escape_json(delivery_failure_reason) << "\""
           << ",\"encoded_video\":[";
    for (std::size_t index = 0; index < encoded_video.size(); ++index) {
        if (index > 0) {
            output << ",";
        }
        const EncodedVideoFrame& frame = encoded_video[index];
        output << "{\"frame_index\":" << frame.frame_index
               << ",\"timestamp_s\":";
        write_double(output, frame.timestamp_s);
        output << ",\"frame_hash\":\"" << escape_json(frame.frame_hash) << "\""
               << ",\"codec\":\"" << escape_json(frame.codec) << "\""
               << ",\"payload_hash\":\"" << escape_json(sha256_hex(frame.payload)) << "\""
               << "}";
    }
    output << "]}";
    return output.str();
}

StreamDeliveryReport stream_telemetry_and_video(
    const std::vector<StreamTelemetrySample>& telemetry,
    const std::vector<RayTracedFrame>& frames,
    LocalTelemetryVideoCollector& collector,
    StreamConfig config) {
    std::vector<EncodedVideoFrame> encoded_video;
    encoded_video.reserve(frames.size());
    for (std::size_t index = 0; index < frames.size(); ++index) {
        encoded_video.push_back(encode_raytraced_frame(frames[index], index));
    }

    if (!collector.available()) {
        return failed_report(telemetry, encoded_video, "collector_unreachable", config.buffer_on_failure);
    }

    StreamDeliveryReport report;
    report.encoded_video = encoded_video;
    report.timestamp_alignment_ok =
        timestamp_aligned(telemetry, frames, config.max_timestamp_skew_s);
    if (!report.timestamp_alignment_ok) {
        report.status = StreamDeliveryStatus::TimestampMisaligned;
        report.delivery_failure_reason = "timestamp_alignment_failed";
        if (config.buffer_on_failure) {
            report.buffered_telemetry_count = telemetry.size();
            report.buffered_video_count = encoded_video.size();
        }
        return report;
    }

    for (const StreamTelemetrySample& sample : telemetry) {
        if (!collector.collect_telemetry(sample)) {
            return failed_report(telemetry, encoded_video, "collector_unreachable", config.buffer_on_failure);
        }
        ++report.sent_telemetry_count;
    }
    for (const EncodedVideoFrame& frame : encoded_video) {
        if (!collector.collect_video(frame)) {
            return failed_report(telemetry, encoded_video, "collector_unreachable", config.buffer_on_failure);
        }
        ++report.sent_video_count;
    }

    report.acked_telemetry_count = collector.telemetry_receipts().size();
    report.acked_video_count = collector.video_receipts().size();
    report.decoded_video_count = static_cast<std::size_t>(std::count_if(
        collector.video_receipts().begin(),
        collector.video_receipts().end(),
        [](const VideoReceipt& receipt) {
            return receipt.decoded;
        }));
    report.status = StreamDeliveryStatus::Delivered;
    return report;
}

} // namespace agbot::flight_sim
