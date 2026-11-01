//! Binary format for baked skies: a magic tag, the atmosphere parameters
//! as f64, then the scattering and sun tables as f32, all little endian.

use crate::sky::atmosphere::Params;
use crate::sky::scattering::{Scattering, CELLS};
use crate::sky::Sky;
use crate::sky::sun::{self, Sun};

const MAGIC: &[u8; 4] = b"SKY1";
const HEADER: usize = 4 + 4 * 8;

pub fn encode(sky: &Sky) -> Vec<u8> {
    let p = sky.params;
    let params = [p.aerosol, p.humidity, p.ozone, p.albedo].map(f64::to_le_bytes);
    let cells = sky.scattering.cells.iter().flatten().flatten();
    let floats = cells.chain(sky.sun.radiance.iter().flatten());
    let tables = floats.flat_map(|&x| (x as f32).to_le_bytes());
    MAGIC.iter().copied().chain(params.concat()).chain(tables).collect()
}

pub fn decode(bytes: &[u8]) -> Sky {
    assert_eq!(&bytes[..4], MAGIC, "not a baked sky");
    let float = |c: &[u8]| f64::from_le_bytes(c.try_into().unwrap());
    let p: Vec<f64> = bytes[4..HEADER].chunks_exact(8).map(float).collect();
    let params = Params { aerosol: p[0], humidity: p[1], ozone: p[2], albedo: p[3] };
    let single = |c: &[u8]| f32::from_le_bytes(c.try_into().unwrap()) as f64;
    let floats: Vec<f64> = bytes[HEADER..].chunks_exact(4).map(single).collect();
    let expected = CELLS * 9 + sun::SIZE * 3;
    assert_eq!(floats.len(), expected, "sky baked with other table sizes");
    let (cells, radiance) = floats.split_at(CELLS * 9);
    let xyz = |c: &[f64]| [c[0], c[1], c[2]];
    let sample = |c: &[f64]| [xyz(&c[0..3]), xyz(&c[3..6]), xyz(&c[6..9])];
    Sky {
        params,
        scattering: Scattering { cells: cells.chunks_exact(9).map(sample).collect() },
        sun: Sun { radiance: radiance.chunks_exact(3).map(xyz).collect() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sky() -> Sky {
        let params = Params { aerosol: 0.2, humidity: 0.7, ozone: 320.0, albedo: 0.3 };
        let value = |i: usize| (i % 1000) as f64 * 0.25;
        let sample = |i: usize| [[value(i), 1.0, 2.0], [3.0, 4.0, 5.0], [6.0, 7.0, value(i + 1)]];
        Sky {
            params,
            scattering: Scattering { cells: (0..CELLS).map(sample).collect() },
            sun: Sun { radiance: (0..sun::SIZE).map(|i| [value(i); 3]).collect() },
        }
    }

    #[test]
    fn encoding_round_trips() {
        let (a, b) = (sky(), decode(&encode(&sky())));
        assert_eq!(a.params, b.params);
        assert_eq!(a.scattering.cells, b.scattering.cells);
        assert_eq!(a.sun.radiance, b.sun.radiance);
    }
}
