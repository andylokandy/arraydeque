use super::CircularBuffer;

pub struct Parameters {
    pub dst: usize,
    pub src: usize,
    pub len: usize,
    pub dst_after_src: bool,
    pub src_pre_wrap_len: usize,
    pub dst_pre_wrap_len: usize,
    pub src_wraps: bool,
    pub dst_wraps: bool,
}

pub struct Wrapping;

impl Wrapping {
    /// Copies a potentially wrapping block of memory len long from src to dest.
    /// (abs(dst - src) + len) must be no larger than cap() (There must be at
    /// most one continuous overlapping region between src and dest).
    pub unsafe fn wrap_copy<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        let Parameters { dst_after_src, src_wraps, dst_wraps, .. } = params;
        match (dst_after_src, src_wraps, dst_wraps) {
            (_, false, false) => Self::none_wrap(buffer, params),
            (false, false, true) => Self::dst_wraps(buffer, params),
            (true, false, true) => Self::dst_after_src_dst_wraps(buffer, params),
            (false, true, false) => Self::src_wraps(buffer, params),
            (true, true, false) => Self::dst_after_src_src_wraps(buffer, params),
            (false, true, true) => Self::src_wraps_dst_wraps(buffer, params),
            (true, true, true) => Self::dst_after_src_src_wraps_dst_wraps(buffer, params),
        }
    }

    unsafe fn none_wrap<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // src doesn't wrap, dst doesn't wrap
        //
        //        S . . .
        // 1 [_ _ A A B B C C _]
        // 2 [_ _ A A A A B B _]
        //            D . . .
        //

        let Parameters { dst, src, len, .. } = params;
        buffer.copy(dst, src, len);
    }

    unsafe fn dst_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // dst before src, src doesn't wrap, dst wraps
        //
        //    S . . .
        // 1 [A A B B _ _ _ C C]
        // 2 [A A B B _ _ _ A A]
        // 3 [B B B B _ _ _ A A]
        //    . .           D .
        //

        let Parameters { dst, src, len, dst_pre_wrap_len, .. } = params;
        buffer.copy(dst, src, dst_pre_wrap_len);
        buffer.copy(0, src + dst_pre_wrap_len, len - dst_pre_wrap_len);
    }

    unsafe fn dst_after_src_dst_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // src before dst, src doesn't wrap, dst wraps
        //
        //              S . . .
        // 1 [C C _ _ _ A A B B]
        // 2 [B B _ _ _ A A B B]
        // 3 [B B _ _ _ A A A A]
        //    . .           D .
        //

        let Parameters { dst, src, len, dst_pre_wrap_len, .. } = params;
        buffer.copy(0, src + dst_pre_wrap_len, len - dst_pre_wrap_len);
        buffer.copy(dst, src, dst_pre_wrap_len);
    }

    unsafe fn src_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // dst before src, src wraps, dst doesn't wrap
        //
        //    . .           S .
        // 1 [C C _ _ _ A A B B]
        // 2 [C C _ _ _ B B B B]
        // 3 [C C _ _ _ B B C C]
        //              D . . .
        //

        let Parameters { dst, src, len, src_pre_wrap_len, .. } = params;
        buffer.copy(dst, src, src_pre_wrap_len);
        buffer.copy(dst + src_pre_wrap_len, 0, len - src_pre_wrap_len);
    }

    unsafe fn dst_after_src_src_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // src before dst, src wraps, dst doesn't wrap
        //
        //    . .           S .
        // 1 [A A B B _ _ _ C C]
        // 2 [A A A A _ _ _ C C]
        // 3 [C C A A _ _ _ C C]
        //    D . . .
        //

        let Parameters { dst, src, len, src_pre_wrap_len, .. } = params;
        buffer.copy(dst + src_pre_wrap_len, 0, len - src_pre_wrap_len);
        buffer.copy(dst, src, src_pre_wrap_len);
    }

    unsafe fn src_wraps_dst_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // dst before src, src wraps, dst wraps
        //
        //    . . .         S .
        // 1 [A B C D _ E F G H]
        // 2 [A B C D _ E G H H]
        // 3 [A B C D _ E G H A]
        // 4 [B C C D _ E G H A]
        //    . .         D . .
        //

        let Parameters { dst, src, len, src_pre_wrap_len, dst_pre_wrap_len, .. } = params;
        debug_assert!(dst_pre_wrap_len > src_pre_wrap_len);
        let delta = dst_pre_wrap_len - src_pre_wrap_len;
        buffer.copy(dst, src, src_pre_wrap_len);
        buffer.copy(dst + src_pre_wrap_len, 0, delta);
        buffer.copy(0, delta, len - dst_pre_wrap_len);
    }

    unsafe fn dst_after_src_src_wraps_dst_wraps<B: CircularBuffer>(buffer: &mut B, params: Parameters) {
        // src before dst, src wraps, dst wraps
        //
        //    . .         S . .
        // 1 [A B C D _ E F G H]
        // 2 [A A B D _ E F G H]
        // 3 [H A B D _ E F G H]
        // 4 [H A B D _ E F F G]
        //    . . .         D .
        //

        let Parameters { dst, src, len, src_pre_wrap_len, dst_pre_wrap_len, .. } = params;
        debug_assert!(src_pre_wrap_len > dst_pre_wrap_len);
        let array_len = buffer.array_len();
        let delta = src_pre_wrap_len - dst_pre_wrap_len;
        buffer.copy(delta, 0, len - src_pre_wrap_len);
        buffer.copy(0, array_len - delta, delta);
        buffer.copy(dst, src, dst_pre_wrap_len);
    }
}
