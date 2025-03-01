use std::mem;

pub fn gcd(mut a: usize, mut b: usize) -> usize {
    if a < b {
        mem::swap(&mut a, &mut b);
    }

    while b > 0 {
        a %= b;
        mem::swap(&mut a, &mut b);
    }

    a
}

pub fn gcd_nums(iter: impl IntoIterator<Item = usize>) -> Option<usize> {
    let mut iter = iter.into_iter();
    let mut result = iter.next()?;
    for v in iter {
        result = gcd(result, v);
    }

    Some(result)
}
