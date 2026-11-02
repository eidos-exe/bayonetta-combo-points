// Native joint transforms use a 16-byte SIMD load/store even though the
// skyline Vector3f binding exposes only three floats. Pass the complete
// allocation, not a pointer derived from its 12-byte xyz subobject.
#[repr(C,align(16))]
pub struct JointVector {pub lanes:[f32;4]}
impl JointVector {
 pub fn new(xyz:[f32;3])->Self {Self{lanes:[xyz[0],xyz[1],xyz[2],0.]}}
 pub fn xyz(&self)->[f32;3] {[self.lanes[0],self.lanes[1],self.lanes[2]]}
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn native_simd_store_fits_without_touching_guard() {
  #[repr(C)] struct Guarded {vector:JointVector,guard:[u32;4]}
  let mut v=Guarded{vector:JointVector::new([0.;3]),guard:[0xa5a5a5a5;4]};
  unsafe {let p=&mut v.vector as *mut JointVector as *mut f32;for i in 0..4 {p.add(i).write(i as f32+1.);}}
  assert_eq!(v.vector.xyz(),[1.,2.,3.]);assert_eq!(v.guard,[0xa5a5a5a5;4]);
 }
 #[test] fn native_vector_has_full_simd_size_and_alignment() {
  assert_eq!(std::mem::size_of::<JointVector>(),16);
  assert_eq!(std::mem::align_of::<JointVector>(),16);
 }
}
