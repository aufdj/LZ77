use std::{
    fs::{File, metadata},
    io::{Read, Write, BufReader, BufWriter, BufRead},
    time::Instant,
    path::Path,
    env,
};

// Convenience functions for buffered I/O -------------------------------------------------------------------------------------------------
#[derive(PartialEq, Eq)]
enum BufferState {
    NotEmpty,
    Empty,
}

trait BufferedRead {
    fn read_byte(&mut self, input: &mut [u8; 1]);
    fn fill_buffer(&mut self) -> BufferState;
}
impl BufferedRead for BufReader<File> {
    fn read_byte(&mut self, input: &mut [u8; 1]) {
        match self.read(input) {
            Ok(_)  => {},
            Err(e) => { 
                println!("Function read_byte failed."); 
                println!("Error: {}", e);
            },
        };
        if self.buffer().len() <= 0 { 
            self.consume(self.capacity()); 
            match self.fill_buf() {
                Ok(_)  => {},
                Err(e) => {
                    println!("Function read_byte failed.");
                    println!("Error: {}", e);
                },
            }
        }
    }
    fn fill_buffer(&mut self) -> BufferState {
        self.consume(self.capacity());
        match self.fill_buf() {
            Ok(_)  => {},
            Err(e) => { 
                println!("Function fill_buffer failed."); 
                println!("Error: {}", e);
            },
        }
        if self.buffer().is_empty() { 
            return BufferState::Empty; 
        }
        BufferState::NotEmpty
    }
}
trait BufferedWrite {
    fn write_byte(&mut self, output: u8);
    fn flush_buffer(&mut self);
}
impl BufferedWrite for BufWriter<File> {
    fn write_byte(&mut self, output: u8) {
        match self.write(&[output]) {
            Ok(_)  => {},
            Err(e) => { 
                println!("Function write_byte failed."); 
                println!("Error: {}", e);
            },
        }
        if self.buffer().len() >= self.capacity() { 
            match self.flush() {
                Ok(_)  => {},
                Err(e) => { 
                    println!("Function write_byte failed."); 
                    println!("Error: {}", e);
                },
            } 
        }
    }
    fn flush_buffer(&mut self) {
        match self.flush() {
            Ok(_)  => {},
            Err(e) => { 
                println!("Function flush_buffer failed."); 
                println!("Error: {}", e);
            },
        }    
    }
}
fn new_input_file(capacity: usize, file_name: &str) -> BufReader<File> {
    BufReader::with_capacity(capacity, File::open(file_name).unwrap())
}
fn new_output_file(capacity: usize, file_name: &str) -> BufWriter<File> {
    BufWriter::with_capacity(capacity, File::create(file_name).unwrap())
}
// ----------------------------------------------------------------------------------------------------------------------------------------

const BUFFER_SIZE: usize = 4096;
const WINDOW_SIZE: usize = 2048;
const MAX_MATCHES: usize = 512;

struct Lz77 {
    window:     [u8; WINDOW_SIZE],
    code_pos:   usize,
    p:          usize,
    file_in:    BufReader<File>,
    file_out:   BufWriter<File>,   
}
impl Lz77 {
    fn new(file_in: BufReader<File>, file_out: BufWriter<File>) -> Lz77 {
        Lz77 {
            window: [0; WINDOW_SIZE],
            code_pos: 0,
            p: 0,
            file_in, file_out,
        }
    }
    fn slide(&mut self, match_len: u16) -> usize {
        // Slide window forward match_len bytes
        for i in 0..match_len {
            self.window[self.p % WINDOW_SIZE] = self.file_in.buffer()[self.code_pos + (i as usize)];
            self.p += 1;
        }

        // Move code_pos forward match_len bytes and check for end of buffer
        self.code_pos += match_len as usize; 
        if self.code_pos >= self.file_in.buffer().len() {
            self.code_pos = 0;
            if self.file_in.fill_buffer() == BufferState::Empty { 
                return 1; 
            }
        }
        0
    }
    fn inc_code_pos(&mut self) -> usize {
        self.code_pos += 1; 
        if self.code_pos >= self.file_in.buffer().len() {
            self.code_pos = 0;
            if self.file_in.fill_buffer() == BufferState::Empty { 
                return 1; 
            }
        }
        0
    }
    fn compress(&mut self) {
        let mut match_offsets = [0u16; MAX_MATCHES];
        let mut match_offset: u16 = 0; 
        let mut match_len: u16 = 1; 
        let mut longest_match_len: u16 = 1;
        let mut match_found = false;

        loop {
            // Find up to MAX_MATCHES matches 
            let mut num_matches: usize = 0;
            for i in (0..self.window.len()).rev() {  
                if self.window[i] != 0 {
                    if self.file_in.buffer()[self.code_pos] == self.window[i] { 
                        match_offsets[num_matches] = i as u16;
                        num_matches += 1;
                        if num_matches >= MAX_MATCHES - 1 { break; }
                        match_found = true;
                    }
                }        
            }
            
            // Find the length for each match and pick the longest one
            for offset in match_offsets.iter() 
            .filter(|&x| 
               *x != 0 
            && *x != (WINDOW_SIZE - 1) as u16) {
                for j in (self.code_pos + 1)..(self.file_in.buffer().len()) { 
                    if self.file_in.buffer()[j] == self.window[((*offset + match_len) as usize) % WINDOW_SIZE] 
                    && match_len < 31 {
                        match_len += 1;    
                    } else { 
                        break; 
                    }  
                }
                if match_len > longest_match_len {
                    longest_match_len = match_len;
                    match_offset = *offset;
                }
                match_len = 1;        
            }

            match_len = longest_match_len;

            match match_offset {
                0..=7 => { 
                    match_found = false; 
                    match_len = 1;
                },
                2047 => { 
                    match_found = false; 
                    match_len = 1;
                },
                _ => {},
            }

            // Write byte literal or pointer and slide code_pos/window forward
            if match_found == false {
                self.file_out.write_byte(0);
                self.file_out.write_byte(self.file_in.buffer()[self.code_pos]);
                if self.slide(match_len) == 1 { break; }
            } else {
                let pointer = ((match_offset & 0x7FF) << 5) + (match_len & 31);
                self.file_out.write_byte((pointer >> 8) as u8);
                self.file_out.write_byte((pointer & 0x00FF) as u8);      
                if self.slide(match_len) == 1 { break; }   
            }

            match_len = 1;                      // Reset variables
            longest_match_len = 1;              //
            match_offset = 0;                   //
            match_found = false;                //
            for i in 0..match_offsets.len() {   //
                match_offsets[i] = 0;           //
            }                                   //
        } 
        self.file_out.flush_buffer();
    }
    fn decompress(&mut self) {  
        let mut match_len: u16 = 1; 
        let mut window_bytes = [0u8; 32];

        loop {
            // Read next two bytes
            let mut pointer = (self.file_in.buffer()[self.code_pos] as u16) * 256;
            if self.inc_code_pos() == 1 { break; }
            pointer += self.file_in.buffer()[self.code_pos] as u16;

            // Byte Literal
            if (pointer >> 8) == 0 {
                self.file_out.write_byte((pointer & 0x00FF) as u8);
                if self.slide(match_len) == 1 { break; }
            // Offset-length pair
            } else {
                let match_offset = (pointer >> 5) & 0x7FF;
                match_len = pointer & 31;

                // Write match to file_out and save bytes to be added to window
                for i in 0..match_len {
                    let byte = self.window[(match_offset + i) as usize % WINDOW_SIZE];
                    self.file_out.write_byte(byte);
                    window_bytes[i as usize] = byte;
                }

                // Slide window forward
                for i in 0..match_len {
                    self.window[self.p % WINDOW_SIZE] = window_bytes[i as usize];
                    self.p += 1;
                }
                if self.inc_code_pos() == 1 { break; }
            }

            match_len = 1;                      // Reset variables
            for i in 0..window_bytes.len() {    //
                window_bytes[i] = 0;            //
            }                                   //
        }
        self.file_out.flush_buffer();
    }
}

fn main() {
    let start_time = Instant::now();
    let args: Vec<String> = env::args().collect();
    let mut file_in = new_input_file(BUFFER_SIZE, &args[2]);
    let file_out = new_output_file(BUFFER_SIZE, &args[3]);
    file_in.fill_buffer();

    match (&args[1]).as_str() {
        "c" => {
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.compress();
            let file_in_size  = metadata(Path::new(&args[2])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
            println!("Finished Compressing");
            println!("{} bytes -> {} bytes in {:.2?}", file_in_size, file_out_size, start_time.elapsed());
        }
        "d" => { 
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.decompress();
            let file_in_size  = metadata(Path::new(&args[2])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
            println!("Finished Decompressing");
            println!("{} bytes -> {} bytes in {:.2?}", file_in_size, file_out_size, start_time.elapsed());
        }
        _ => { 
            println!("To compress: c input output.");
            println!("To decompress: c input output.");
        }
    }   
}    
