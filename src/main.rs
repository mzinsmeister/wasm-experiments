use std::{sync::Arc, u64::MAX};

use wasm_encoder::{CodeSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection, Instruction, MemArg, MemorySection, Module, TypeSection, ValType};
use wasmtime::{Extern, LinearMemory, Memory, MemoryCreator, MemoryType};

const MAX_PAGES: u64 = 281_474_976_710_656u64;

struct HostPseudoLinearMemory();

unsafe impl LinearMemory for HostPseudoLinearMemory {
    fn byte_size(&self) -> usize {
        MAX_PAGES as usize
    }

    fn maximum_byte_size(&self) -> Option<usize> {
        Some(MAX_PAGES as usize)
    }

    fn grow_to(&mut self, _new_size: usize) -> wasmtime::Result<()> {
        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        0 as *mut u8
    }

    fn wasm_accessible(&self) -> std::ops::Range<usize> {
        0..usize::MAX
    }
}

// A memory creator that just maps the entire 64-bit host address space into a pseudo linear-memory wasm address space.
struct HostMemoryCreator {}

unsafe impl MemoryCreator for HostMemoryCreator {
    fn new_memory(&self, _ty: MemoryType, _minimum: usize, _maximum: Option<usize>, _reserved_size_in_bytes: Option<usize>, _guard_size_in_bytes: usize) -> std::result::Result<Box<(dyn LinearMemory + 'static)>, std::string::String> {
        let nullptr_box = Box::new(HostPseudoLinearMemory { });
        Ok(nullptr_box)
    }
}

fn main() {

    // Define a binary wasm function that reads a 64 bit integer from memory 2 at a given 64-bit offset (memory64 wasm)
    // increments it and returns the previous value.

    let mut module = Module::new();

    // Encode the type section.
    let mut types = TypeSection::new();
    let params = vec![ValType::I64];
    let results = vec![ValType::I64];
    types.function(params, results);
    module.section(&types);

    // Encode the function section.
    let mut functions = FunctionSection::new();
    let type_index = 0;
    functions.function(type_index);
    module.section(&functions);

    // Encode the memories section
    let mut memories = MemorySection::new();
    memories.memory(wasm_encoder::MemoryType{ minimum: MAX_PAGES - 1, maximum: None, shared: false, memory64: true });
    module.section(&memories);

    // Encode the export section.
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("f", ExportKind::Func, 0);
    module.section(&exports);

    // Encode the code section.
    let mut codes = CodeSection::new();
    let locals = vec![];
    let mut f = Function::new(locals);

    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I64Load(MemArg { align: 3, offset: 0, memory_index: 0 }));
    f.instruction(&Instruction::LocalTee(0));
    f.instruction(&Instruction::I64Const(1));
    f.instruction(&Instruction::I64Add);
    f.instruction(&Instruction::I64Store(MemArg { align: 3, offset: 0, memory_index: 0 }));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::End);
    codes.function(&f);
    module.section(&codes);

    // Extract the encoded Wasm bytes for this module.
    let wasm_bytes = module.finish();

    // Initialize Wasmtime runtime

    let memory_creator = HostMemoryCreator {};

    let mut config = wasmtime::Config::new();

    config.static_memory_forced(true);
    config.wasm_memory64(true);
    config.static_memory_maximum_size(u64::MAX);
    config.wasm_multi_memory(true);
    config.with_host_memory(Arc::new(memory_creator));
    config.static_memory_guard_size(0);
    config.dynamic_memory_guard_size(0);
    config.guard_before_linear_memory(false);

    let engine = wasmtime::Engine::new(&config).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();


    // Call the exported function
    let f = instance.get_func(&mut store, "f").unwrap();
    let mut results = vec![wasmtime::Val::I64(0)];
    let value = Box::new(42u64);
    let value_ptr = &*value as *const u64;
    f.call(&mut store, &[wasmtime::Val::I64(value_ptr as i64)], &mut results).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].unwrap_i64(), 42);

    println!("Memory at location was {}", results[0].unwrap_i64() as u64);
    println!("Memory at location is {}", value);
}
