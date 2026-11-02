// The camera export returns a SIMD vector, not a C struct of four floats.
// Match the ABI documented by smash-rs app/camera.rs and cpp/simd.rs.
#[derive(Clone,Copy)]#[repr(simd)]struct CameraVector {lanes:[f32;4]}
extern "C" {#[link_name="_ZN3app6camera16get_camera_rangeEv"]fn native_camera_range()->CameraVector;}
pub unsafe fn bounds()->[f32;4]{{let v=native_camera_range();[core::intrinsics::simd::simd_extract(v,0),core::intrinsics::simd::simd_extract(v,1),core::intrinsics::simd::simd_extract(v,2),core::intrinsics::simd::simd_extract(v,3)]}}
