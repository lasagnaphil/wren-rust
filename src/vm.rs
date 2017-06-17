use std::mem;
use std::slice;
use std::ffi::{CStr, CString};
use libc::{c_int, c_char};
use ffi;
use {ErrorType, InterpretResult, Type, Pointer, ReallocateFn, LoadModuleFn, BindForeignMethodFn,
     BindForeignClassFn, WriteFn, ErrorFn};

// These default functions mimic those used in the Wren CLI interpreter.

fn default_write(_: &mut VM, text: &str) {
    print!("{}", text);
}

fn default_error(_: &mut VM, _type: ErrorType, module: &str, line: i32, message: &str) {
    match _type {
        ErrorType::Compile => println!("[{} line {}] {}", module, line, message),
        ErrorType::Runtime => println!("{}", message),
        ErrorType::StackTrace => println!("[{} line {}] in {}", module, line, message),
    }
}

fn default_load_module(_: &mut VM, name: &str) -> Option<String> {
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::Read;

    let mut buffer = String::new();

    // Look for a file named [name].wren.
    let mut name_path = PathBuf::new();
    name_path.push(name);
    name_path.set_extension("wren");
    let result = File::open(&name_path).map(|mut f| f.read_to_string(&mut buffer));
    if result.is_ok() {
        return Some(buffer);
    }

    // If that fails, treat [name] as a directory and look for module.wren in there.
    name_path.set_extension("");
    name_path.push("module");
    name_path.set_extension("wren");
    buffer.clear();
    let result = File::open(&name_path).map(|mut f| f.read_to_string(&mut buffer));
    if result.is_ok() { Some(buffer) } else { None }
}

/// Wrapper around a `WrenConfiguration`.
///
/// Refer to `wren.h` for info on each field.
pub struct Configuration(ffi::WrenConfiguration);

impl Configuration {
    /// Create a new Configuration using `wrenInitConfiguration`.
    ///
    /// This also sets the printing and module loading functions to mimic those used in the CLI interpreter.
    pub fn new() -> Configuration {
        let mut raw: ffi::WrenConfiguration = unsafe { mem::uninitialized() };
        unsafe { ffi::wrenInitConfiguration(&mut raw) }
        let mut cfg = Configuration(raw);
        cfg.set_write_fn(wren_write_fn!(default_write));
        cfg.set_error_fn(wren_error_fn!(default_error));
        cfg.set_load_module_fn(wren_load_module_fn!(default_load_module));
        cfg
    }
    pub fn set_reallocate_fn(&mut self, f: ReallocateFn) {
        self.0.reallocate_fn = f;
    }

    pub fn set_load_module_fn(&mut self, f: LoadModuleFn) {
        self.0.load_module_fn = f;
    }

    pub fn set_bind_foreign_method_fn(&mut self, f: BindForeignMethodFn) {
        self.0.bind_foreign_method_fn = f;
    }

    pub fn set_bind_foreign_class_fn(&mut self, f: BindForeignClassFn) {
        self.0.bind_foreign_class_fn = f;
    }

    pub fn set_write_fn(&mut self, f: WriteFn) {
        self.0.write_fn = f;
    }

    pub fn set_error_fn(&mut self, f: ErrorFn) {
        self.0.error_fn = f;
    }

    pub fn set_initial_heap_size(&mut self, size: usize) {
        self.0.initial_heap_size = size;
    }

    pub fn set_min_heap_size(&mut self, size: usize) {
        self.0.min_heap_size = size;
    }

    pub fn set_heap_growth_percent(&mut self, percent: i32) {
        self.0.heap_growth_percent = percent;
    }

    pub fn set_user_data(&mut self, data: Pointer) {
        self.0.user_data = data;
    }
}

/// Wrapper around a `WrenHandle`.
#[derive(Copy, Clone)]
pub struct Handle(*mut ffi::WrenHandle);

/// Wrapper around a `WrenVM`.
///
/// Refer to wren.h for info on each function.
pub struct VM {
    raw: *mut ffi::WrenVM,
    owned: bool,
}

impl VM {
    /// Create a new VM.
    pub fn new(cfg: Configuration) -> VM {
        let mut cfg = cfg;
        let raw = unsafe { ffi::wrenNewVM(&mut cfg.0) };
        VM { raw, owned: true }
    }

    /// Create a wrapper around an existing WrenVM pointer.
    ///
    /// This is mainly used by function wrapping macros.
    pub unsafe fn from_ptr(ptr: *mut ffi::WrenVM) -> VM {
        VM {
            raw: ptr,
            owned: false,
        }
    }

    /// Maps to `wrenCollectGarbage`.
    pub fn collect_garbage(&mut self) {
        unsafe { ffi::wrenCollectGarbage(self.raw) }
    }

    /// Maps to `wrenInterpret`.
    pub fn interpret(&mut self, source: &str) -> InterpretResult {
        let source_cstr = CString::new(source).unwrap();
        unsafe { ffi::wrenInterpret(self.raw, source_cstr.as_ptr()) }
    }

    /// Maps to `wrenMakeCallHandle`.
    pub fn make_call_handle(&mut self, signature: &str) -> Handle {
        let signature_cstr = CString::new(signature).unwrap();
        let handle = unsafe { ffi::wrenMakeCallHandle(self.raw, signature_cstr.as_ptr()) };
        Handle(handle)
    }

    /// Maps to `wrenCall`.
    pub fn call(&mut self, method: Handle) -> InterpretResult {
        unsafe { ffi::wrenCall(self.raw, method.0) }
    }

    /// Maps to `wrenReleaseHandle`.
    pub fn release_handle(&mut self, handle: Handle) {
        unsafe { ffi::wrenReleaseHandle(self.raw, handle.0) }
    }

    /// Maps to `wrenGetSlotCount`.
    pub fn get_slot_count(&mut self) -> i32 {
        unsafe { ffi::wrenGetSlotCount(self.raw) }
    }

    /// Maps to `wrenEnsureSlots`.
    pub fn ensure_slots(&mut self, num_slots: i32) {
        unsafe { ffi::wrenEnsureSlots(self.raw, num_slots) }
    }

    /// Maps to `wrenGetSlotType`.
    pub fn get_slot_type(&mut self, slot: i32) -> Type {
        unsafe { ffi::wrenGetSlotType(self.raw, slot) }
    }

    /// Maps to `wrenGetSlotBool`.
    pub fn get_slot_bool(&mut self, slot: i32) -> bool {
        unsafe { ffi::wrenGetSlotBool(self.raw, slot) != 0 }
    }

    /// Maps to `wrenGetSlotBytes`.
    pub fn get_slot_bytes(&mut self, slot: i32) -> &[u8] {
        let mut length = unsafe { mem::uninitialized() };
        let ptr = unsafe { ffi::wrenGetSlotBytes(self.raw, slot, &mut length) };
        unsafe { slice::from_raw_parts(ptr as *const u8, length as usize) }
    }

    /// Maps to `wrenGetSlotDouble`.
    pub fn get_slot_double(&mut self, slot: i32) -> f64 {
        unsafe { ffi::wrenGetSlotDouble(self.raw, slot) }
    }

    /// Maps to `wrenGetSlotForeign`.
    pub fn get_slot_foreign(&mut self, slot: i32) -> Pointer {
        unsafe { ffi::wrenGetSlotForeign(self.raw, slot) }
    }

    /// Maps to `wrenGetSlotString`.
    pub fn get_slot_string(&mut self, slot: i32) -> &str {
        let ptr = unsafe { ffi::wrenGetSlotString(self.raw, slot) };
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }

    /// Maps to `wrenGetSlotHandle`.
    pub fn get_slot_handle(&mut self, slot: i32) -> Handle {
        let handle = unsafe { ffi::wrenGetSlotHandle(self.raw, slot) };
        Handle(handle)
    }

    /// Maps to `wrenSetSlotBool`.
    pub fn set_slot_bool(&mut self, slot: i32, value: bool) {
        unsafe { ffi::wrenSetSlotBool(self.raw, slot, value as c_int) }
    }

    /// Maps to `wrenSetSlotBytes`.
    pub fn set_slot_bytes(&mut self, slot: i32, bytes: &[u8]) {
        let ptr = bytes.as_ptr() as *const c_char;
        let len = bytes.len();
        unsafe { ffi::wrenSetSlotBytes(self.raw, slot, ptr, len) }
    }

    /// Maps to `wrenSetSlotDouble`.
    pub fn set_slot_double(&mut self, slot: i32, value: f64) {
        unsafe { ffi::wrenSetSlotDouble(self.raw, slot, value) }
    }

    /// Maps to `wrenSetSlotNewForeign`.
    pub fn set_slot_new_foreign(&mut self, slot: i32, class_slot: i32, size: usize) -> Pointer {
        unsafe { ffi::wrenSetSlotNewForeign(self.raw, slot, class_slot, size) }
    }

    /// Maps to `wrenSetSlotNewList`.
    pub fn set_slot_new_list(&mut self, slot: i32) {
        unsafe { ffi::wrenSetSlotNewList(self.raw, slot) }
    }

    /// Maps to `wrenSetSlotNull`.
    pub fn set_slot_null(&mut self, slot: i32) {
        unsafe { ffi::wrenSetSlotNull(self.raw, slot) }
    }

    /// Maps to `wrenSetSlotString`.
    pub fn set_slot_string(&mut self, slot: i32, s: &str) {
        let cstr = CString::new(s).unwrap();
        unsafe { ffi::wrenSetSlotString(self.raw, slot, cstr.as_ptr()) }
    }

    /// Maps to `wrenSetSlotHandle`.
    pub fn set_slot_handle(&mut self, slot: i32, handle: Handle) {
        unsafe { ffi::wrenSetSlotHandle(self.raw, slot, handle.0) }
    }

    /// Maps to `wrenGetListCount`.
    pub fn get_list_count(&mut self, slot: i32) -> i32 {
        unsafe { ffi::wrenGetListCount(self.raw, slot) }
    }

    /// Maps to `wrenGetListElement`.
    pub fn get_list_element(&mut self, list_slot: i32, index: i32, element_slot: i32) {
        unsafe { ffi::wrenGetListElement(self.raw, list_slot, index, element_slot) }
    }

    // Maybe rename this to be consistent with get_list_element?
    /// Maps to `wrenInsertInList`.
    pub fn insert_in_list(&mut self, list_slot: i32, index: i32, element_slot: i32) {
        unsafe { ffi::wrenInsertInList(self.raw, list_slot, index, element_slot) }
    }

    /// Maps to `wrenGetVariable`.
    pub fn get_variable(&mut self, module: &str, name: &str, slot: i32) {
        let module_cstr = CString::new(module).unwrap();
        let name_cstr = CString::new(name).unwrap();
        unsafe { ffi::wrenGetVariable(self.raw, module_cstr.as_ptr(), name_cstr.as_ptr(), slot) }
    }

    /// Maps to `wrenAbortFiber`.
    pub fn abort_fiber(&mut self, slot: i32) {
        unsafe { ffi::wrenAbortFiber(self.raw, slot) }
    }

    /// Maps to `wrenGetUserData`.
    pub fn get_user_data(&mut self) -> Pointer {
        unsafe { ffi::wrenGetUserData(self.raw) }
    }

    /// Maps to `wrenSetUserData`.
    pub fn set_user_data(&mut self, data: Pointer) {
        unsafe { ffi::wrenSetUserData(self.raw, data) }
    }
}

impl Drop for VM {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ffi::wrenFreeVM(self.raw) }
        }
    }
}
