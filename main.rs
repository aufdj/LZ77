use std::{
    fs::{File, metadata},
    io::{Write, BufReader, BufWriter, BufRead},
    time::Instant,
    path::Path,
    env,
};

// Convenience functions for buffered I/O
#[derive(PartialEq, Eq)]
enum BufferState {
    NotEmpty,
    Empty,
}

trait BufferedRead {
    fn fill_buffer(&mut self) -> BufferState;
}
impl BufferedRead for BufReader<File> {
    fn fill_buffer(&mut self) -> BufferState {
        self.consume(self.capacity());
        if let Err(e) = self.fill_buf() {
            println!("Function fill_buffer failed.");
            println!("Error: {}", e);
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
        if let Err(e) = self.write(&[output]) {
            println!("Function write_byte failed.");
            println!("Error: {}", e);
        }
        
        if self.buffer().len() >= self.capacity() {
            if let Err(e) = self.flush() {
                println!("Function write_byte failed.");
                println!("Error: {}", e);
            }
        }
    }
    fn flush_buffer(&mut self) {
        if let Err(e) = self.flush() {
            println!("Function flush_buffer failed.");
            println!("Error: {}", e);
        }    
    }
}
fn new_input_file(capacity: usize, file_name: &str) -> BufReader<File> {
    BufReader::with_capacity(
        capacity, File::open(file_name).unwrap()
    )
}
fn new_output_file(capacity: usize, file_name: &str) -> BufWriter<File> {
    BufWriter::with_capacity(
        capacity, File::create(file_name).unwrap()
    )
}


const WIN_SIZE: usize = 2048; // Window Size
const MAX_MCHS: usize = 512;  // Maximum number of matches

struct Match {
    pub offset: u16,
    pub len:    u16,
}
impl Match {
    fn new(offset: u16) -> Match {
        Match {
            offset,
            len: 1,
        }
    }
    fn update(&mut self, offset: u16, len: u16) {
        self.offset = offset;
        self.len = len;
    }
    fn reset(&mut self) {
        self.offset = 0;
        self.len = 1;
    }
}
impl Default for Match {
    fn default() -> Match {
        Match {
            offset: 0,
            len:    1,
        }
    }
}

struct Lz77 {
    win:       [u8; WIN_SIZE],
    code_pos:  usize,
    p:         usize,
    file_in:   BufReader<File>,
    file_out:  BufWriter<File>,   
}
impl Lz77 {
    fn new(file_in: BufReader<File>, file_out: BufWriter<File>) -> Lz77 {
        Lz77 {
            win:       [0; WIN_SIZE],
            code_pos:  0,
            p:         0,
            file_in,   
            file_out,
        }
    }

    fn slide(&mut self, mch_len: u16) -> BufferState {
        // Slide win forward mch_len bytes
        for i in 0..mch_len {
            let mch_byte = self.file_in.buffer()[self.code_pos + (i as usize)];
            self.win[self.p % WIN_SIZE] = mch_byte;
            self.p += 1;
        }

        // Move code_pos forward mch_len bytes and check for end of buffer
        self.code_pos += mch_len as usize;
        if self.code_pos >= self.file_in.buffer().len() {
            self.code_pos = 0;
            return self.file_in.fill_buffer()   
        }
        BufferState::NotEmpty
    }

    fn inc_code_pos(&mut self) -> BufferState {
        self.code_pos += 1; 
        if self.code_pos >= self.file_in.buffer().len() {
            self.code_pos = 0;
            return self.file_in.fill_buffer()   
        }
        BufferState::NotEmpty
    }

    fn compress(&mut self) {
        let mut matches = Vec::<Match>::new();  // Candidate matches
        let mut longest_mch = Match::default(); // Longest match found

        loop {
            let curr_byte = self.file_in.buffer()[self.code_pos];

            // Find up to MAX_MCHS matches 
            for i in (0..self.win.len()).rev() {
                if self.win[i] == curr_byte { 
                    matches.push(Match::new(i as u16));
                    if matches.len() == MAX_MCHS { 
                        break; 
                    }
                }       
            }
            
            // Find the length for each match and pick the longest one
            for mch in matches.iter_mut() {
                // Increase match length if byte at next 
                // code pos 'c' equals next byte in window 'w'
                for c in self.file_in.buffer().iter().skip(self.code_pos + 1) {
                    let win_pos = (mch.offset + mch.len) as usize;
                    let w = self.win[win_pos % WIN_SIZE];

                    if *c == w && mch.len < 31 {
                        mch.len += 1; 
                    } 
                    else { 
                        break; 
                    }  
                }
            
                if mch.len > longest_mch.len {
                    longest_mch.update(mch.offset, mch.len);
                }      
            }

            if longest_mch.offset <= 7 {
                longest_mch.len = 1;
            }

            // Write byte literal and slide window forward
            if longest_mch.len == 1 {
                self.file_out.write_byte(0);
                self.file_out.write_byte(self.file_in.buffer()[self.code_pos]);
                if self.slide(longest_mch.len) == BufferState::Empty { 
                    break; 
                }
            } 
            // Write pointer and slide window forward
            else {
                let ptr = ((longest_mch.offset & 0x7FF) << 5) + (longest_mch.len & 31);
                self.file_out.write_byte((ptr >> 8) as u8);
                self.file_out.write_byte((ptr & 0x00FF) as u8);      
                if self.slide(longest_mch.len) == BufferState::Empty { 
                    break; 
                }   
            }

            matches.clear();
            longest_mch.reset();
        } 
        self.file_out.flush_buffer();
    }

    fn decompress(&mut self) {  
        let mut mch = Match::default();
        let mut win_bytes = [0u8; 32];

        loop {
            // Read next two bytes
            let mut ptr = (self.file_in.buffer()[self.code_pos] as u16) * 256;
            if self.inc_code_pos() == BufferState::Empty { 
                break; 
            }
            ptr += self.file_in.buffer()[self.code_pos] as u16;

            // Byte Literal
            if (ptr >> 8) == 0 { 
                self.file_out.write_byte((ptr & 0x00FF) as u8);
                if self.slide(mch.len) == BufferState::Empty { 
                    break; 
                }
            } 
            // Offset-length pair
            else { 
                mch.offset = (ptr >> 5) & 0x7FF;
                mch.len = ptr & 31;

                // Write match to file_out and save bytes to be added to window
                for i in 0..mch.len {
                    let byte = self.win[(mch.offset + i) as usize % WIN_SIZE];
                    self.file_out.write_byte(byte);
                    win_bytes[i as usize] = byte;
                }

                // Slide window forward
                for i in 0..mch.len {
                    self.win[self.p % WIN_SIZE] = win_bytes[i as usize];
                    self.p += 1;
                }
                if self.inc_code_pos() == BufferState::Empty {
                    break; 
                }
            }

            mch.reset();
            win_bytes.map(|_| 0);
        }
        self.file_out.flush_buffer();
    }
}

fn main() {
    let start_time = Instant::now();
    let args = env::args().skip(1).collect::<Vec<String>>();
    let mut file_in = new_input_file(4096, &args[1]);
    let file_out = new_output_file(4096, &args[2]);
    file_in.fill_buffer();

    match (&args[0]).as_str() {
        "c" => {
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.compress();
            let file_in_size  = metadata(Path::new(&args[1])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[2])).unwrap().len();
            println!("Finished Compressing");
            println!("{} bytes -> {} bytes in {:.2?}", 
                file_in_size, file_out_size, start_time.elapsed());
        }
        "d" => { 
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.decompress();
            let file_in_size  = metadata(Path::new(&args[1])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[2])).unwrap().len();
            println!("Finished Decompressing");
            println!("{} bytes -> {} bytes in {:.2?}", 
                file_in_size, file_out_size, start_time.elapsed());
        }
        _ => { 
            println!("To compress: c input output.");
            println!("To decompress: c input output.");
        }
    }   
}    