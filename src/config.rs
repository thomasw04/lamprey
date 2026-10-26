use core::fmt::Display;

pub mod parse;

#[derive(Debug, PartialEq)]
pub struct Component<'a> {
    pub path: &'a str,
    pub cmdline: &'a str,
}

pub enum Directive<'a> {
    Comment(&'a str),
    BootDriver(Component<'a>),
    CpuDriver(Component<'a>),
    Module(Component<'a>),
    Stack(usize),
}

impl Display for Directive<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Directive::Comment(comment) => write!(f, "#{}", comment),
            Directive::BootDriver(component) => {
                write!(f, "boot-driver {} {}", component.path, component.cmdline)
            }
            Directive::Module(component) => {
                write!(f, "module {} {}", component.path, component.cmdline)
            }
            Directive::CpuDriver(component) => {
                write!(f, "cpu-driver {} {}", component.path, component.cmdline)
            }
            Directive::Stack(size) => write!(f, "stack {}", size),
        }
    }
}
