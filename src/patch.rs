use std::io::{self, Read, Write, Seek, Cursor};
use std::cmp::min;

use bzip2::bufread::BzDecoder;

use core::{
    Command,
    CommandReader,
    Header,
};

fn read_paired_bufs<F, R0: Read, R1: Read>(
    mut size: u64,
    mut r0: R0,
    mut r1: R1,
    mut f: F
) -> io::Result<()>
    where F: FnMut(&mut [u8], &mut [u8]) -> io::Result<()>
{
    let mut buf0 = [0u8; 1024];
    let mut buf1 = [0u8; 1024];

    let (mut p0, mut p1) = (0, 0);
    let mut base = 0;

    while size > 0 {
        // println!("base {}", base);
        let avail = min(buf0.len() as u64, size) as usize;
        // println!("avail {} p0 {}", avail, p0);
        if p0 < avail {
            let s0 = r0.read(&mut buf0[p0..avail])?;
            p0 += s0;
            // println!("s0 {} p0 {}", s0, p0);
        }

        let avail = min(buf1.len() as u64, size) as usize;
        // println!("avail {} p1 {}", avail, p1);
        if p1 < avail {
            let s1 = r1.read(&mut buf1[p1..avail])?;
            p1 += s1;
            // println!("s1 {} p1 {}", s1, p1);
        }

        let pmin = min(p0, p1);

        f(&mut buf0[base..pmin], &mut buf1[base..pmin])?;

        if p0 < p1 {
            for i in pmin..p1 {
                buf1[i - pmin] = buf1[i];
            }
        } else if p1 < p0 {
            for i in pmin..p0 {
                buf0[i - pmin] = buf0[i];
            }
        }

        p0 -= pmin;
        p1 -= pmin;

        let processed = pmin - base;

        // println!("size {} processed {}", size, processed);

        size -= processed as u64;
        base = 0;
    }

    Ok(())
}

fn read_size_from<F, R: Read>(mut size: u64, mut r: R, mut f: F) -> io::Result<()>
    where F: FnMut(&mut [u8]) -> io::Result<()>
{
    let mut buf = [0u8; 1024];

    let mut p = 0;
    let mut base = 0;

    while size > 0 {
        let avail = min(buf.len() as u64, size) as usize;
        if p < avail {
            let s = r.read(&mut buf[p..avail])?;
            p += s;
        }

        f(&mut buf[base..p])?;

        base = 0;
        size -= p as u64;
    }

    Ok(())
}

struct Patcher<DeltaR, ExtraR, OldRS, NewW> {
    delta: DeltaR,
    extra: ExtraR,
    old: OldRS,
    new: NewW,
}

impl<DeltaR, ExtraR, OldRS, NewW> Patcher<DeltaR, ExtraR, OldRS, NewW>
    where
        DeltaR: Read,
        ExtraR: Read,
        OldRS: Read+Seek,
        NewW: Write
{
    fn apply(&mut self, c: &Command) -> io::Result<()> {
        self.append_delta(c.bytewise_add_size)?;
        self.append_extra(c.extra_append_size)?;
        self.seek_old(c.oldfile_seek_offset)?;
        Ok(())
    }

    fn append_delta(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_paired_bufs(size, &mut self.old, &mut self.delta, |o, d| {
            for i in 0..o.len() {
                o[i] = o[i].wrapping_add(d[i]);
            }
            new.write_all(&o)
        })
    }

    fn append_extra(&mut self, size: u64) -> io::Result<()> {
        let new = &mut self.new;
        read_size_from(size, &mut self.extra, |e| {
            new.write_all(&e)
        })
    }

    fn seek_old(&mut self, size: i64) -> io::Result<()> {
        self.old.seek(io::SeekFrom::Current(size)).map(|_|())
    }

    fn check_written_size(&self, _: u64) -> io::Result<()> {
        // TODO: return an error if we haven't written the expected size to the output.
        Ok(())
    }
}

pub fn apply<OldRS, NewW>(patch: &[u8], old: OldRS, new: NewW) -> io::Result<()>
    where
        OldRS: Read+Seek,
        NewW: Write
{
    let (header, body) = patch.split_at(32);

    let header = Header::read(&header)?;

    let (command_data, rest) = body.split_at(header.compressed_commands_size as usize);
    let (delta_data, extra_data) = rest.split_at(header.compressed_delta_size as usize);

    let command_stream = BzDecoder::new(Cursor::new(command_data));

    let commands = CommandReader::new(command_stream);

    let delta = BzDecoder::new(Cursor::new(delta_data));
    let extra = BzDecoder::new(Cursor::new(extra_data));

    let mut patcher = Patcher {
        delta: delta,
        extra: extra,
        old: old,
        new: new
    };

    for cmd in commands {
        println!("cmd {:?}", cmd);
        patcher.apply(&(cmd?))?;
    }

    patcher.check_written_size(header.new_file_size)?;

    Ok(())
}

#[cfg(test)]
mod tests {
}