#include "wrapper.h"

#include <memory>
#include <string>

#include <openvpn/client/cliconstants.hpp>
#include <openvpn/io/io.hpp>
#include <openvpn/log/logsimple.hpp>
#include <openvpn/options/merge.hpp>

struct OvpnProfileMergeResult {
    std::string status;
    std::string error;
    std::string content;
};

extern "C" OvpnProfileMergeResult *ovpn_profile_merge(
    const char *profile_content,
    const char *reference_dir) {
    if (profile_content == nullptr || reference_dir == nullptr) {
        return nullptr;
    }

    try {
        auto result = std::make_unique<OvpnProfileMergeResult>();
        const openvpn::ProfileMergeFromString merge(
            profile_content,
            reference_dir,
            openvpn::ProfileMerge::FOLLOW_FULL,
            openvpn::ProfileParseLimits::MAX_LINE_SIZE,
            openvpn::ProfileParseLimits::MAX_PROFILE_SIZE);

        result->status = merge.status_string();
        if (merge.status() == openvpn::ProfileMerge::MERGE_SUCCESS) {
            result->content = merge.profile_content();
        } else {
            result->error = merge.error();
        }
        return result.release();
    } catch (...) {
        return nullptr;
    }
}

extern "C" const char *ovpn_profile_merge_status(const OvpnProfileMergeResult *result) {
    return result == nullptr ? nullptr : result->status.c_str();
}

extern "C" const char *ovpn_profile_merge_error(const OvpnProfileMergeResult *result) {
    return result == nullptr ? nullptr : result->error.c_str();
}

extern "C" const char *ovpn_profile_merge_content(const OvpnProfileMergeResult *result) {
    return result == nullptr ? nullptr : result->content.c_str();
}

extern "C" void ovpn_profile_merge_result_free(OvpnProfileMergeResult *result) {
    delete result;
}
