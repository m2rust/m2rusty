//! `std::sort` / `std::ranges::sort` as libstdc++ implements it (introsort
//! with a median-of-three pivot, insertion sort below 16 elements, heap
//! sort when the depth limit is hit). It is not stable, and the order of
//! equal keys decides the order of equally-numbered area objects, so it is
//! reproduced exactly.

const THRESHOLD: usize = 16;

/// Sorts `v` by `key(a) < key(b)` the way libstdc++'s `std::sort` does.
pub fn sort_by_less<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool + Copy) {
    if v.len() > 1 {
        let depth = 2 * (usize::BITS - 1 - v.len().leading_zeros()) as usize;
        introsort_loop(v, depth, less);
        final_insertion_sort(v, less);
    }
}

fn introsort_loop<T: Clone>(mut v: &mut [T], mut depth: usize, less: impl Fn(&T, &T) -> bool + Copy) {
    while v.len() > THRESHOLD {
        if depth == 0 {
            heap_sort(v, less);
            return;
        }
        depth -= 1;
        let cut = partition_pivot(v, less);
        let (left, right) = v.split_at_mut(cut);
        introsort_loop(right, depth, less);
        v = left;
    }
}

fn move_median_to_first<T>(v: &mut [T], a: usize, b: usize, c: usize, less: impl Fn(&T, &T) -> bool) {
    // result = first; swap(first, median(a, b, c))
    let m = if less(&v[a], &v[b]) {
        if less(&v[b], &v[c]) {
            b
        } else if less(&v[a], &v[c]) {
            c
        } else {
            a
        }
    } else if less(&v[a], &v[c]) {
        a
    } else if less(&v[b], &v[c]) {
        c
    } else {
        b
    };
    v.swap(0, m);
}

fn partition_pivot<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool + Copy) -> usize {
    let mid = v.len() / 2;
    let last = v.len() - 1;
    move_median_to_first(v, 1, mid, last, less);
    // unguarded partition of (first + 1, last) around *first
    let pivot = v[0].clone();
    let mut first = 1usize;
    let mut last = v.len();
    loop {
        while less(&v[first], &pivot) {
            first += 1;
        }
        last -= 1;
        while less(&pivot, &v[last]) {
            last -= 1;
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

fn insertion_sort<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool) {
    for i in 1..v.len() {
        if less(&v[i], &v[0]) {
            let val = v[i].clone();
            v[..=i].rotate_right(1);
            v[0] = val;
        } else {
            let val = v[i].clone();
            let mut j = i;
            while less(&val, &v[j - 1]) {
                v[j] = v[j - 1].clone();
                j -= 1;
            }
            v[j] = val;
        }
    }
}

fn final_insertion_sort<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool + Copy) {
    if v.len() > THRESHOLD {
        insertion_sort(&mut v[..THRESHOLD], less);
        for i in THRESHOLD..v.len() {
            let val = v[i].clone();
            let mut j = i;
            while less(&val, &v[j - 1]) {
                v[j] = v[j - 1].clone();
                j -= 1;
            }
            v[j] = val;
        }
    } else {
        insertion_sort(v, less);
    }
}

fn sift_down<T: Clone>(v: &mut [T], mut hole: usize, len: usize, value: T, less: impl Fn(&T, &T) -> bool + Copy) {
    // std::__adjust_heap
    let top = hole;
    let mut child = hole;
    while child < (len.saturating_sub(1)) / 2 {
        child = 2 * (child + 1);
        if less(&v[child], &v[child - 1]) {
            child -= 1;
        }
        v[hole] = v[child].clone();
        hole = child;
    }
    if len % 2 == 0 && child == (len - 2) / 2 {
        child = 2 * (child + 1);
        v[hole] = v[child - 1].clone();
        hole = child - 1;
    }
    // push_heap
    let mut parent = (hole.wrapping_sub(1)) / 2;
    while hole > top && less(&v[parent], &value) {
        v[hole] = v[parent].clone();
        hole = parent;
        parent = (hole.wrapping_sub(1)) / 2;
    }
    v[hole] = value;
}

fn heap_sort<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool + Copy) {
    let len = v.len();
    // make_heap
    if len >= 2 {
        let mut parent = (len - 2) / 2;
        loop {
            let value = v[parent].clone();
            sift_down(v, parent, len, value, less);
            if parent == 0 {
                break;
            }
            parent -= 1;
        }
    }
    // sort_heap
    let mut last = len;
    while last > 1 {
        last -= 1;
        let value = v[last].clone();
        v[last] = v[0].clone();
        sift_down(&mut v[..last], 0, last, value, less);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_key() {
        let mut state = 12345u32;
        for n in [0usize, 1, 2, 15, 16, 17, 100, 1000] {
            let mut v: Vec<(u32, u32)> = (0..n)
                .map(|i| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    ((state >> 24) % 7, i as u32)
                })
                .collect();
            sort_by_less(&mut v, |a, b| a.0 < b.0);
            assert!(v.windows(2).all(|w| w[0].0 <= w[1].0));
        }
    }

    #[test]
    fn heap_fallback_sorts() {
        let mut v: Vec<u32> = (0..200).map(|i| (i * 7919) % 101).collect();
        heap_sort(&mut v, |a, b| a < b);
        assert!(v.windows(2).all(|w| w[0] <= w[1]));
    }
}
