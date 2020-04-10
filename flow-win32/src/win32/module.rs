/*
pub mod iter;
pub use iter::*;

pub mod export;
pub use export::*;

pub mod section;
pub use section::*;
*/
use crate::error::{Error, Result};
use log::trace;

use flow_core::*;

use crate::offsets::Win32Offsets;
use crate::win32::*;

use pelite::{self, image::GUID, pe64::debug::CodeView, PeView};

#[derive(Debug, Clone)]
pub struct Win32Module {
    peb_module: Address,
    parent_eprocess: Address, // parent "reference"

    base: Address,
    size: Length,
    name: String,
    // exports
    // sections
}

impl Win32Module {
    pub fn try_with_peb<T, U>(
        mem: &mut T,
        process: &U,
        offsets: &Win32Offsets,
        peb_module: Address,
    ) -> Result<Self>
    where
        T: VirtualMemoryTrait,
        U: ProcessTrait,
    {
        let mut proc_reader = VirtualMemory::with_proc_arch(
            mem,
            process.sys_arch(),
            process.proc_arch(),
            process.dtb(),
        );

        let (ldr_data_base, ldr_data_size, ldr_data_name) =
            match process.proc_arch().instruction_set {
                InstructionSet::X64 => (
                    offsets.ldr_data_base_x64,
                    offsets.ldr_data_size_x64,
                    offsets.ldr_data_name_x64,
                ),
                InstructionSet::X86 => (
                    offsets.ldr_data_base_x86,
                    offsets.ldr_data_size_x86,
                    offsets.ldr_data_name_x86,
                ),
                _ => return Err(Error::new("invalid architecture")),
            };
        trace!("ldr_data_base={:x}", ldr_data_base);
        trace!("ldr_data_size={:x}", ldr_data_size);
        trace!("ldr_data_name={:x}", ldr_data_name);

        let base = proc_reader.virt_read_addr(peb_module + ldr_data_base)?;
        trace!("base={:x}", base);

        let size = match process.proc_arch().instruction_set {
            InstructionSet::X64 => {
                let mut s = 0u64;
                proc_reader.virt_read(peb_module + ldr_data_size, &mut s)?;
                Length::from(s)
            }
            InstructionSet::X86 => {
                let mut s = 0u32;
                proc_reader.virt_read(peb_module + ldr_data_size, &mut s)?;
                Length::from(s)
            }
            _ => return Err(Error::new("invalid architecture")),
        };
        trace!("size={:x}", size);

        let name = match process.proc_arch().instruction_set {
            InstructionSet::X64 => {
                proc_reader.virt_read_unicode_string(peb_module + offsets.ldr_data_name_x64)?
            }
            InstructionSet::X86 => {
                proc_reader.virt_read_unicode_string(peb_module + offsets.ldr_data_name_x86)?
            }
            _ => return Err(Error::new("invalid architecture")),
        };
        trace!("name={}", name);

        Ok(Self {
            peb_module,
            parent_eprocess: process.address(),
            base,
            size,
            name,
        })
    }

    pub fn try_with_name<T, U>(
        mem: &mut T,
        process: &U,
        offsets: &Win32Offsets,
        name: &str,
    ) -> Result<Self>
    where
        T: VirtualMemoryTrait,
        U: ProcessTrait + Win32Process,
    {
        process
            .peb_list(mem)?
            .iter()
            .map(|peb| Win32Module::try_with_peb(mem, process, offsets, *peb))
            .filter_map(Result::ok)
            .inspect(|p| trace!("{:x} {}", p.base(), p.name()))
            .filter(|p| p.name() == name)
            .nth(0)
            .ok_or_else(|| Error::new(format!("unable to find process {}", name)))
    }

    // read_image() - reads the entire image into memory
    pub fn read_image<T, U>(&self, mem: &mut T, process: &U) -> Result<Vec<u8>>
    where
        T: VirtualMemoryTrait,
        U: ProcessTrait + Win32Process,
    {
        let mut proc_reader = VirtualMemory::with_proc_arch(
            mem,
            process.sys_arch(),
            process.proc_arch(),
            process.dtb(),
        );

        let mut probe_buf = vec![0; Length::from_kb(4).as_usize()];
        proc_reader.virt_read_raw(self.base, &mut probe_buf)?;

        let pe_probe = match PeView::from_bytes(&probe_buf) {
            Ok(pe) => {
                trace!("found pe header.");
                pe
            }
            Err(e) => {
                trace!(
                    "pe header at offset {:x} could not be probed: {:?}",
                    self.base,
                    e
                );
                return Err(Error::from(e));
            }
        };

        let opt_header = pe_probe.optional_header();
        let size_of_image = match opt_header {
            pelite::Wrap::T32(opt32) => opt32.SizeOfImage,
            pelite::Wrap::T64(opt64) => opt64.SizeOfImage,
        };
        if size_of_image == 0 {
            return Err(Error::new("unable to read size_of_image"));
        }
        info!("found pe header with a size of {} bytes.", size_of_image);

        let mut buf = vec![0; size_of_image as usize];
        proc_reader.virt_read_raw(self.base, &mut buf)?;
        Ok(buf)
    }
}

impl ModuleTrait for Win32Module {
    fn address(&self) -> Address {
        self.peb_module
    }

    fn parent_process(&self) -> Address {
        self.parent_eprocess
    }

    fn base(&self) -> Address {
        self.base
    }

    fn size(&self) -> Length {
        self.size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}
