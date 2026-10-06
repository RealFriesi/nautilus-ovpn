#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OvpnProfileMergeResult OvpnProfileMergeResult;

OvpnProfileMergeResult *ovpn_profile_merge(const char *profile_content, const char *reference_dir);
const char *ovpn_profile_merge_status(const OvpnProfileMergeResult *result);
const char *ovpn_profile_merge_error(const OvpnProfileMergeResult *result);
const char *ovpn_profile_merge_content(const OvpnProfileMergeResult *result);
void ovpn_profile_merge_result_free(OvpnProfileMergeResult *result);

#ifdef __cplusplus
}
#endif
