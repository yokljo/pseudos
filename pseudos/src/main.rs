use std::cmp::Ordering;

use libpseudos::dos_event_handler::{DosEventHandler, MachineType, PortStates};
use libpseudos::dos_file_system::StandardDosFileSystem;
use libpseudos::exe_loader::MzHeader;
use xachtsechs::machine8086::Machine8086;
use xachtsechs::types::{InterruptResult, Reg, RegHalf, StepResult};

use sdl2::image::{LoadTexture, INIT_PNG};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Rect;
use sdl2::render::{WindowCanvas, Texture};
use sdl2::audio::AudioSpecDesired;

use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;

fn get_ms_from_duration(duration: std::time::Duration) -> usize {
	(duration.as_secs() * 1000) as usize + duration.subsec_millis() as usize
}

pub fn vga_colour_to_rgb(colour: u8) -> (u8, u8, u8) {
	match colour {
		0x0 => (0x00, 0x00, 0x00),
		0x1 => (0x00, 0x00, 0xAA),
		0x2 => (0x00, 0xAA, 0x00),
		0x3 => (0x00, 0xAA, 0xAA),
		0x4 => (0xAA, 0x00, 0x00),
		0x5 => (0xAA, 0x00, 0xAA),
		0x6 => (0xAA, 0x55, 0x00),
		0x7 => (0xAA, 0xAA, 0xAA),
		0x8 => (0x55, 0x55, 0x55),
		0x9 => (0x55, 0x55, 0xFF),
		0xA => (0x55, 0xFF, 0x55),
		0xB => (0x55, 0xFF, 0xFF),
		0xC => (0xFF, 0x55, 0x55),
		0xD => (0xFF, 0x55, 0xFF),
		0xE => (0xFF, 0xFF, 0x55),
		0xF => (0xFF, 0xFF, 0xFF),
		_ => (0, 0, 0)
	}
}

struct DosConsole {
	machine: Machine8086,
	dos_event_handler: DosEventHandler,
	current_run_time_ms: usize,
}

impl DosConsole {
	fn draw_screen(&mut self, canvas: &mut WindowCanvas, dosfont_tex: &mut Texture, redraw_all: bool) {
		let screen_mem = &self.machine.memory[0xb8000..0xb8000+0x1000];
		let screen_width = 80;
		let screen_height = 25;
		for y in 0 .. screen_height {
			for x in 0 .. screen_width {
				let char_index = (x + (y * screen_width)) * 2;
				let ref char_code = screen_mem[char_index];
				let ref colour = screen_mem[char_index + 1];
				let colour_fore = colour & 0x0f;
				let mut colour_back = (colour & 0xf0) >> 4;
				
				let mut blinking = false;
				
				if colour_back >= 8 {
					colour_back -= 8;
					blinking = true;
				}
				
				let fore_rgb = vga_colour_to_rgb(colour_fore);
				let back_rgb = vga_colour_to_rgb(colour_back);

				let char_rect = Rect::new(8 * (*char_code as i32), 0, 8, 14);
				let dest_rect = Rect::new(8 * (x as i32), 14 * (y as i32), 8, 14);

				// Draw the character background:
				canvas.set_draw_color(sdl2::pixels::Color::RGB(back_rgb.0, back_rgb.1, back_rgb.2));
				canvas.fill_rect(dest_rect).ok();

				if !blinking || self.current_run_time_ms % 450 < 225 {
					// Draw the character foreground:
					dosfont_tex.set_color_mod(fore_rgb.0, fore_rgb.1, fore_rgb.2);
					canvas.copy(&dosfont_tex, Some(char_rect), Some(dest_rect)).expect("Render failed");
				}
			}
		}
	}
	
	fn run(&mut self) {
		let mut step_count = 0;

		//
		// Init SDL2.
		//
		
		let scale = 2;

		let sdl_context = sdl2::init().unwrap();

		//
		// Init video.
		//

		let render_width = 640;
		let render_height = 350;
		
		let sdl_video = sdl_context.video().unwrap();
		let _sdl_image = sdl2::image::init(INIT_PNG).unwrap();
		let window = sdl_video.window("PseuDOS", render_width * scale, render_height * scale)
			.position_centered()
			//.fullscreen_desktop()
			.build()
			.unwrap();
			
		let (window_width, window_height) = window.size();

		let mut canvas = window.into_canvas().software().build().unwrap();
		let texture_creator = canvas.texture_creator();

		let dosfont_file = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/dosfont.png"));
		let mut dosfont_tex = texture_creator.load_texture(dosfont_file).unwrap();

		let mut running = true;

		canvas.set_scale(scale as f32, scale as f32).ok();
		canvas.set_viewport(Rect::new(((window_width / scale) as i32 / 2 - render_width as i32 / 2) as i32, ((window_height / scale) as i32 / 2 - render_height as i32 / 2) as i32, render_width, render_height));

		//sdl_context.mouse().show_cursor(false);

		let start_time_ms = get_ms_from_duration(SystemTime::now().duration_since(UNIX_EPOCH).unwrap());
		let mut last_time_ms = start_time_ms;

		self.draw_screen(&mut canvas, &mut dosfont_tex, true);

		while running {
			for event in sdl_context.event_pump().unwrap().poll_iter() {
				match event {
					Event::Quit{..} => {
						running = false;
					}
					_ => {}
				}
			}
			
			self.machine.interrupt_on_next_step(0x08);
			self.dos_event_handler.seconds_since_start += 54.9451/1000.;
			self.dos_event_handler.set_cga_vertial_retrace(true);
			
			let num_opcodes_to_exec = 10000;
			for _ in 0..num_opcodes_to_exec {
				match self.machine.step(&mut self.dos_event_handler) {
					StepResult::Interrupt(InterruptResult::Wait) => {
						break;
					}
					_ => {}
				}
				step_count += 1;
			}
			
			/*if machine.number_of_parsed_instructions > 797780 {
				println!("MEM: {:?}", &machine.memory[0xb8000..0xb8000+0x1000]);
				panic!();
			}*/
			
			self.draw_screen(&mut canvas, &mut dosfont_tex, false);

			self.current_run_time_ms += 20;
			canvas.present();
		}
	}
}

fn main() {
	let mut file = std::fs::File::open("./junk/dos/ZZT.EXE").unwrap();
	let exe_header = MzHeader::parse(&mut file).unwrap();
	println!("{:#?}", exe_header);
	let mut machine = Machine8086::new(1024*1024*1);
	exe_header.load_into_machine(&mut machine, &mut file);
	let mut event_handler = DosEventHandler {
		machine_type: MachineType::EGA,
		video_mode: MachineType::EGA.lookup_video_mode(3).unwrap(),
		port_states: PortStates::new(),
		file_system: Box::new(StandardDosFileSystem::new("./junk/dos".into())),
		seconds_since_start: 0.,
	};
	event_handler.init_machine(&mut machine);

    let mut console = DosConsole {
		machine,
		dos_event_handler: event_handler,
		current_run_time_ms: 0,
    };
    console.run();
}
