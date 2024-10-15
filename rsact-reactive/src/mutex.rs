// use core::sync::atomic::{AtomicBool, Ordering};
// use lock_api::{GuardSend, RawMutex};

// mod my_spin_lock {
//     pub struct RawSpinLock(AtomicBool);

//     unsafe impl RawMutex for RawSpinLock {
//         const INIT: Self = Self(AtomicBool::new(false));

//         type GuardMarker = GuardSend;

//         fn lock(&self) {
//             while !self.try_lock() {
//                 core::hint::spin_loop();
//             }
//         }

//         fn try_lock(&self) -> bool {
//             self.0
//                 .compare_exchange(
//                     false,
//                     true,
//                     Ordering::Acquire,
//                     Ordering::Relaxed,
//                 )
//                 .is_ok()
//             // self.0.swap(true, Ordering::Acquire)
//         }

//         unsafe fn unlock(&self) {
//             self.0.store(false, Ordering::Release);
//         }
//     }

//     pub type SpinLock<T> = lock_api::Mutex<RawSpinLock, T>;
// }

// #[cfg(feature = "spin")]
// pub type Mutex<T> = spin::Mutex<T>;

// #[cfg(feature = "mutex-critical-section")]
// type Raw<T> = lock_api::Mutex<critical_section_mutex::Raw, T>;

// We don't need Mutex from std, with std we can just use stable `thread_local`
// #[cfg(feature = "std")]
// pub type Mutex<T> = std::sync::Mutex<T>;
