pub mod bios_loader;
pub mod dos_event_handler;
pub mod dos_error_codes;
pub mod dos_file_system;
pub mod exe_loader;

// https://en.wikipedia.org/wiki/Program_Segment_Prefix
// https://toonormal.com/2018/06/07/notes-ms-dos-dev-for-intel-8086-cpus-using-a-modern-pc/
// - "DOS programs require that all programs start at the 256 byte boundary"
// https://www.daniweb.com/programming/software-development/threads/291076/whats-org-100h
