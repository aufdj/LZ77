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
    BufReader::with_capacity(
        capacity, File::open(file_name).unwrap()
    )
}
fn new_output_file(capacity: usize, file_name: &str) -> BufWriter<File> {
    BufWriter::with_capacity(
        capacity, File::create(file_name).unwrap()
    )
}
// ----------------------------------------------------------------------------------------------------------------------------------------

const WIN_SIZE: usize = 2048; // Window Size
const MAX_MCHS: usize = 512; // Maximum number of matches

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
            win: [0; WIN_SIZE],
            code_pos: 0,
            p: 0,
            file_in, file_out,
        }
    }
    fn slide(&mut self, mch_len: u16) -> usize {
        // Slide win forward mch_len bytes
        for i in 0..mch_len {
            self.win[self.p % WIN_SIZE] = self.file_in.buffer()[self.code_pos + (i as usize)];
            self.p += 1;
        }

        // Move code_pos forward mch_len bytes and check for end of buffer
        self.code_pos += mch_len as usize; 
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
        let mut mch_offs = [0u16; MAX_MCHS]; // Offsets of matches
        let mut mch_off: u16 = 0; // Offset with longest length
        let mut mch_len: u16 = 1; // Length of mch_off
        let mut longest_mch_len: u16 = 1; // Temp current longest length
        let mut mch_found = false; // 

        loop {
            // Find up to MAX_MCHS matches 
            let mut num_matches: usize = 0;
            for i in (0..self.win.len()).rev() {  
                if self.win[i] != 0 {
                    if self.file_in.buffer()[self.code_pos] == self.win[i] { 
                        mch_offs[num_matches] = i as u16;
                        num_matches += 1;
                        if num_matches >= MAX_MCHS - 1 { break; }
                        mch_found = true;
                    }
                }        
            }
            
            // Find the length for each match and pick the longest one
            for off in mch_offs.iter()
            .filter(|&&x| x != 0 && x != (WIN_SIZE - 1) as u16) {
                // Increase match length if byte at next code pos 'c' 
                // equals next byte in window 'w'
                for c in self.file_in.buffer().iter().skip(self.code_pos+1) {
                    let w = self.win[((*off + mch_len) as usize) % WIN_SIZE];
                    if *c == w && mch_len < 31 { mch_len += 1; } 
                    else { break; }  
                }
            
                if mch_len > longest_mch_len {
                    longest_mch_len = mch_len;
                    mch_off = *off;
                }
                mch_len = 1;        
            }

            mch_len = longest_mch_len;

            match mch_off {
                0..=7 => { 
                    mch_found = false; 
                    mch_len = 1;
                },
                2047 => { 
                    mch_found = false; 
                    mch_len = 1;
                },
                _ => {},
            }

            // Write byte literal or ptr and slide code_pos/win forward
            if mch_found == false {
                self.file_out.write_byte(0);
                self.file_out.write_byte(self.file_in.buffer()[self.code_pos]);
                if self.slide(mch_len) == 1 { break; }
            } 
            else {
                let ptr = ((mch_off & 0x7FF) << 5) + (mch_len & 31);
                self.file_out.write_byte((ptr >> 8) as u8);
                self.file_out.write_byte((ptr & 0x00FF) as u8);      
                if self.slide(mch_len) == 1 { break; }   
            }

            mch_len = 1;                  // Reset variables
            longest_mch_len = 1;          //
            mch_off = 0;                  //
            mch_found = false;            //
            for i in 0..mch_offs.len() {  //
                mch_offs[i] = 0;          //
            }                             //
        } 
        self.file_out.flush_buffer();
    }
    fn decompress(&mut self) {  
        let mut mch_len: u16 = 1; 
        let mut win_bytes = [0u8; 32];

        loop {
            // Read next two bytes
            let mut ptr = (self.file_in.buffer()[self.code_pos] as u16) * 256;
            if self.inc_code_pos() == 1 { break; }
            ptr += self.file_in.buffer()[self.code_pos] as u16;

            
            if (ptr >> 8) == 0 { // Byte Literal
                self.file_out.write_byte((ptr & 0x00FF) as u8);
                if self.slide(mch_len) == 1 { break; }
            } 
            else { // Offset-length pair
                let mch_off = (ptr >> 5) & 0x7FF;
                mch_len = ptr & 31;

                // Write match to file_out and save bytes to be added to window
                for i in 0..mch_len {
                    let byte = self.win[(mch_off + i) as usize % WIN_SIZE];
                    self.file_out.write_byte(byte);
                    win_bytes[i as usize] = byte;
                }

                // Slide window forward
                for i in 0..mch_len {
                    self.win[self.p % WIN_SIZE] = win_bytes[i as usize];
                    self.p += 1;
                }
                if self.inc_code_pos() == 1 { break; }
            }

            mch_len = 1;                  // Reset variables
            for i in 0..win_bytes.len() { //
                win_bytes[i] = 0;         //
            }                             //
        }
        self.file_out.flush_buffer();
    }
}

fn main() {
    let start_time = Instant::now();
    let args: Vec<String> = env::args().collect();
    let mut file_in = new_input_file(4096, &args[2]);
    let file_out = new_output_file(4096, &args[3]);
    file_in.fill_buffer();

    match (&args[1]).as_str() {
        "c" => {
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.compress();
            let file_in_size  = metadata(Path::new(&args[2])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
            println!("Finished Compressing");
            println!("{} bytes -> {} bytes in {:.2?}", 
                file_in_size, file_out_size, start_time.elapsed());
        }
        "d" => { 
            let mut lz77 = Lz77::new(file_in, file_out);
            lz77.decompress();
            let file_in_size  = metadata(Path::new(&args[2])).unwrap().len();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
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
