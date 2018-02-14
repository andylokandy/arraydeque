use super::CircularBuffer;

pub struct Parameters {
    pub index: usize,
    pub internal_index: usize,
    pub distance_to_tail: usize,
    pub distance_to_head: usize,
}

pub struct Contiguous;

impl Contiguous {
    #[inline]
    pub unsafe fn insert<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { index, distance_to_tail, distance_to_head, ..} = params;
        match distance_to_tail <= distance_to_head {
            true if index == 0 => Self::closer_to_tail_at_zero(buffer),
            true => Self::closer_to_tail(buffer, params),
            false => Self::closer_to_head(buffer, params),
        }
    }

    #[inline]
    unsafe fn closer_to_tail_at_zero<B: CircularBuffer>(buffer: &mut B) {
        // contiguous, insert closer to tail, at zero:
        //
        //       T
        //       I             H
        //      [A o o o o o o . . . . . . . . .]
        //
        //                       H         T
        //      [A o o o o o o o . . . . . I]
        //

        let new_tail = B::wrap_sub(buffer.tail(), 1);
        buffer.set_tail(new_tail)
    }

    #[inline]
    unsafe fn closer_to_tail<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // contiguous, insert closer to tail:
        //
        //             T   I         H
        //      [. . . o o A o o o o . . . . . .]
        //
        //           T               H
        //      [. . o o I A o o o o . . . . . .]
        //           M M
        //
        // contiguous, insert closer to tail and tail is 0:
        //
        //
        //       T   I         H
        //      [o o A o o o o . . . . . . . . .]
        //
        //                       H             T
        //      [o I A o o o o o . . . . . . . o]
        //       M                             M

        let Parameters { index, ..} = params;

        let tail = buffer.tail();
        let new_tail = B::wrap_sub(buffer.tail(), 1);

        buffer.copy(new_tail, tail, 1);
        // Already moved the tail, so we only copy `index - 1` elements.
        buffer.copy(tail, tail + 1, index - 1);

        buffer.set_tail(new_tail);
    }

    #[inline]
    unsafe fn closer_to_head<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        //  contiguous, insert closer to head:
        //
        //             T       I     H
        //      [. . . o o o o A o o . . . . . .]
        //
        //             T               H
        //      [. . . o o o o I A o o . . . . .]
        //                       M M M

        let Parameters { internal_index, ..} = params;
        let head = buffer.head();
        buffer.copy(internal_index + 1, internal_index, head - internal_index);
        let new_head = B::wrap_add(buffer.head(), 1);
        buffer.set_head(new_head);
    }
}

pub struct Discontiguous;

impl Discontiguous {
    #[inline]
    pub unsafe fn insert<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { internal_index, distance_to_tail, distance_to_head, ..} = params;
        match (distance_to_tail <= distance_to_head, internal_index >= buffer.tail()) {
            (true, true) => Self::closer_to_tail_tail_section(buffer, params),
            (true, false) => Self::closer_to_tail(buffer, params),
            (false, true) => Self::closer_to_head_tail_section(buffer, params),
            (false, false) => Self::closer_to_head_head_section(buffer, params),
        }
    }

    #[inline]
    unsafe fn closer_to_tail<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { internal_index, ..} = params;

        if internal_index == 0 {
            Self::closer_to_tail_head_section_at_zero(buffer);
        } else {
            Self::closer_to_tail_head_section(buffer, params);
        }
    }

    #[inline]
    unsafe fn closer_to_tail_tail_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, insert closer to tail, tail section:
        //
        //                   H         T   I
        //      [o o o o o o . . . . . o o A o o]
        //
        //                   H       T
        //      [o o o o o o . . . . o o I A o o]
        //                           M M

        let Parameters { index, ..} = params;
        let tail = buffer.tail();
        buffer.copy(tail - 1, tail, index);
        buffer.set_tail(tail - 1);
    }

    #[inline]
    unsafe fn closer_to_head_tail_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, insert closer to head, tail section:
        //
        //           H             T         I
        //      [o o . . . . . . . o o o o o A o]
        //
        //             H           T
        //      [o o o . . . . . . o o o o o I A]
        //       M M M                         M

        let Parameters { internal_index, ..} = params;
        let array_len = buffer.array_len();

        // copy elements up to new head
        let head = buffer.head();
        buffer.copy(1, 0, head);

        // copy last element into empty spot at bottom of buffer
        buffer.copy(0, array_len - 1, 1);

        // move elements from internal_index to end forward not including ^ element
        buffer.copy(internal_index + 1, internal_index, array_len - 1 - internal_index);

        buffer.set_head(head + 1);
    }

    #[inline]
    unsafe fn closer_to_tail_head_section_at_zero<B: CircularBuffer>(buffer: &mut B) {
        // discontiguous, insert is closer to tail, head section,
        // and is at index zero in the internal buffer:
        //
        //       I                   H     T
        //      [A o o o o o o o o o . . . o o o]
        //
        //                           H   T
        //      [A o o o o o o o o o . . o o o I]
        //                               M M M

        let array_len = buffer.array_len();

        // copy elements up to new tail
        let tail = buffer.tail();
        buffer.copy(tail - 1, tail, array_len - tail);

        // copy last element into empty spot at bottom of buffer
        buffer.copy(array_len - 1, 0, 1);

        buffer.set_tail(tail - 1);
    }

    #[inline]
    unsafe fn closer_to_tail_head_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, insert closer to tail, head section:
        //
        //             I             H     T
        //      [o o o A o o o o o o . . . o o o]
        //
        //                           H   T
        //      [o o I A o o o o o o . . o o o o]
        //       M M                     M M M M

        let Parameters { internal_index, ..} = params;
        let array_len = buffer.array_len();

        let tail = buffer.tail();
        // copy elements up to new tail
        buffer.copy(tail - 1, tail, array_len - tail);

        // copy last element into empty spot at bottom of buffer
        buffer.copy(array_len - 1, 0, 1);

        // move elements from internal_index-1 to end forward not including ^ element
        buffer.copy(0, 1, internal_index - 1);

        buffer.set_tail(tail - 1);
    }

    #[inline]
    unsafe fn closer_to_head_head_section<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // discontiguous, insert closer to head, head section:
        //
        //               I     H           T
        //      [o o o o A o o . . . . . . o o o]
        //
        //                     H           T
        //      [o o o o I A o o . . . . . o o o]
        //                 M M M

        let Parameters { internal_index, ..} = params;
        let head = buffer.head();
        buffer.copy(internal_index + 1, internal_index, head - internal_index);
        buffer.set_head(head + 1);
    }
}
