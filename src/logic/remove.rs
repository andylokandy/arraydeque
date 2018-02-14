use super::CircularBuffer;

pub struct Parameters {
    pub index: usize,
    pub internal_index: usize,
    pub distance_to_tail: usize,
    pub distance_to_head: usize,
}

pub struct Contiguous;

impl Contiguous {
    pub unsafe fn remove<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { distance_to_tail, distance_to_head, ..} = params;
        match distance_to_tail <= distance_to_head {
            true => Self::closer_to_tail(buffer, params),
            false => Self::closer_to_head(buffer, params),
        }
    }

    #[inline]
    unsafe fn closer_to_tail<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // contiguous, remove closer to tail:
        //
        //             T   R         H
        //      [. . . o o x o o o o . . . . . .]
        //
        //               T           H
        //      [. . . . o o o o o o . . . . . .]
        //               M M

        let Parameters { index, ..} = params;
        let tail = buffer.tail();
        buffer.copy(tail + 1, tail, index);
        buffer.set_tail(tail + 1);
    }

    #[inline]
    unsafe fn closer_to_head<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // contiguous, remove closer to head:
        //
        //             T       R     H
        //      [. . . o o o o x o o . . . . . .]
        //
        //             T           H
        //      [. . . o o o o o o . . . . . . .]
        //                     M M

        let Parameters { internal_index, ..} = params;
        let head = buffer.head();
        buffer.copy(internal_index, internal_index + 1, head - internal_index - 1);
        buffer.set_head(head - 1);
    }
}

pub struct Discontiguous;

impl Discontiguous {
    #[inline]
    pub unsafe fn remove<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { internal_index, distance_to_tail, distance_to_head, ..} = params;
        match (distance_to_tail <= distance_to_head, internal_index >= buffer.tail()) {
            (true, true) => Self::closer_to_tail_tail_section(buffer, params),
            (true, false) => Self::closer_to_tail_head_section(buffer, params),
            (false, true) => Self::closer_to_head_tail_section(buffer, params),
            (false, false) => Self::closer_to_head_head_section(buffer, params),
        }
    }

    #[inline]
    unsafe fn closer_to_tail_tail_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, remove closer to tail, tail section:
        //
        //                   H         T   R
        //      [o o o o o o . . . . . o o x o o]
        //
        //                   H           T
        //      [o o o o o o . . . . . . o o o o]
        //                               M M

        let Parameters { index, ..} = params;
        let tail = buffer.tail();
        buffer.copy(tail + 1, tail, index);
        let new_tail = B::wrap_add(buffer.tail(), 1);
        buffer.set_tail(new_tail);
    }

    #[inline]
    unsafe fn closer_to_head_tail_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, remove closer to head, tail section:
        //
        //             H           T         R
        //      [o o o . . . . . . o o o o o x o]
        //
        //           H             T
        //      [o o . . . . . . . o o o o o o o]
        //       M M                         M M
        //
        // or quasi-discontiguous, remove next to head, tail section:
        //
        //       H                 T         R
        //      [. . . . . . . . . o o o o o x o]
        //
        //                         T           H
        //      [. . . . . . . . . o o o o o o .]
        //                                   M

        let Parameters { internal_index, ..} = params;
        let array_len = buffer.array_len();

        // draw in elements in the tail section
        buffer.copy(internal_index, internal_index + 1, array_len - internal_index - 1);

        // Prevents underflow.
        if buffer.head() != 0 {
            // copy first element into empty spot
            buffer.copy(array_len - 1, 0, 1);

            // move elements in the head section backwards
            let head = buffer.head();
            buffer.copy(0, 1, head - 1);
        }

        let new_head = B::wrap_sub(buffer.head(), 1);
        buffer.set_head(new_head);
    }

    #[inline]
    unsafe fn closer_to_tail_head_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, remove closer to tail, head section:
        //
        //           R               H     T
        //      [o o x o o o o o o o . . . o o o]
        //
        //                           H       T
        //      [o o o o o o o o o o . . . . o o]
        //       M M M                       M M

        let Parameters { internal_index, ..} = params;
        let array_len = buffer.array_len();

        let tail = buffer.tail();
        // draw in elements up to internal_index
        buffer.copy(1, 0, internal_index);

        // copy last element into empty spot
        buffer.copy(0, array_len - 1, 1);

        // move elements from tail to end forward, excluding the last one
        buffer.copy(tail + 1, tail, array_len - tail - 1);

        let new_tail = B::wrap_add(tail, 1);
        buffer.set_tail(new_tail);
    }

    #[inline]
    unsafe fn closer_to_head_head_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, remove closer to head, head section:
        //
        //               R     H           T
        //      [o o o o x o o . . . . . . o o o]
        //
        //                   H             T
        //      [o o o o o o . . . . . . . o o o]
        //               M M

        let Parameters { internal_index, ..} = params;
        let head = buffer.head();
        buffer.copy(internal_index, internal_index + 1, head - internal_index - 1);
        buffer.set_head(head - 1);
    }
}
