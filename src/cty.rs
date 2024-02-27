use anyhow::{anyhow, Result};
use rmp::{
    decode::{self, Bytes, RmpRead},
    encode, Marker,
};

use crate::util::UNKNOWN_STRING;

fn peek_marker(bytes: &mut Bytes) -> Result<Marker> {
    let mut bytes = bytes.clone();

    let marker = decode::read_marker(&mut bytes).map_err(|_| anyhow!("expected marker"))?;
    Ok(marker)
}

fn peek_str_len(bytes: &mut Bytes) -> Result<usize> {
    let mut bytes = bytes.clone();

    let len = decode::read_str_len(&mut bytes).map_err(|_| anyhow!("expected string"))?;
    Ok(len as usize)
}

pub fn encode_unknown_string_values(bytes: Vec<u8>) -> Result<Vec<u8>> {
    let mut result: Vec<u8> = Vec::with_capacity(bytes.len());

    let mut work_bytes = Bytes::from(bytes.as_slice());

    loop {
        if work_bytes.remaining_slice().len() == 0 {
            break;
        }

        let Ok(marker) = peek_marker(&mut work_bytes) else {
            result.push(work_bytes.read_u8()?);
            continue;
        };

        match marker {
            Marker::FixStr(_) | Marker::Str8 | Marker::Str16 | Marker::Str32 => {
                let mut buf = vec![0u8; peek_str_len(&mut work_bytes)?];
                let str: &str = decode::read_str(&mut work_bytes, &mut buf)
                    .map_err(|_| anyhow!("expected string"))?;

                if str == UNKNOWN_STRING {
                    result.push(Marker::FixExt1.to_u8());
                    result.push(0x00);
                    result.push(0x00);
                } else {
                    let mut data = Vec::new();
                    encode::write_str(&mut data, str).map_err(|_| anyhow!("expected string"))?;
                    result.extend(data);
                }
            }
            _ => {
                result.push(work_bytes.read_u8()?);
            }
        };
    }

    Ok(result)
}

pub fn decode_unknown_string_values(bytes: Vec<u8>) -> Result<Vec<u8>> {
    let mut result: Vec<u8> = Vec::with_capacity(bytes.len());

    let mut work_bytes = Bytes::from(bytes.as_slice());

    loop {
        if work_bytes.remaining_slice().len() == 0 {
            break;
        }

        let Ok(marker) = peek_marker(&mut work_bytes) else {
            result.push(work_bytes.read_u8()?);
            continue;
        };

        match marker {
            Marker::FixExt1 => {
                let _ = work_bytes.read_u8()?;
                let _ = work_bytes.read_u8()?;
                let _ = work_bytes.read_u8()?;

                let mut data = Vec::new();
                encode::write_str(&mut data, UNKNOWN_STRING)
                    .map_err(|_| anyhow!("expected string"))?;
                result.extend(data);
            }
            _ => {
                result.push(work_bytes.read_u8()?);
            }
        };
    }

    Ok(result)
}
