use crate::icon_set::icon_set;

icon_set! {
    SystemIcon "system" 0x60 [
        5 {ac: 0x30},
        6 {ac: 0x60},
        7 {ac: 0x40},
        8 {ac: 0x3f},
        9 {ac: 0x7f},
        10,
        11,
        12,
        13 {ac: 0x80},
        14,
        15 {ac: 0x30},
        16,
        17,
        18,
        19,
        20,
        21,
        22,
        23,
        24,
    ] {
        ArrowExpand: "arrow-expand" {
            ac: 0x30,
        } [
            6 {ac: 0xa0}
        ],
        ArrowExpandAll: "arrow-expand-all" {
            ac: 0x30,
        } [
            6 {ac: 0xa0}
        ],
        // TODO: Review on 6px
        ArrowBottomLeft: "arrow-bottom-left" {
            ac: 0x40,
        } [
            5 {ac: 0x20},
        ],
        ArrowBottomRight: "arrow-bottom-right" {
            ac: 0x60,
        } [
            5 {ac: 0x20},
        ],
        ArrowTopLeft: "arrow-top-left" {
            ac: 0x40,
        } [
            5 {ac: 0x20},
        ],
        ArrowTopRight: "arrow-top-right" {
            ac: 0x40,
        } [
            5 {ac: 0x20},
        ],
        // TODO: ac + scale on small sizes
        ArrowLeft: "arrow-left" {
            ac: 0x20,
        } [
            7 {ac: 0x10},
        ],
        ArrowRight: "arrow-right" {
            ac: 0x20,
        } [
            7 {ac: 0x10},
        ],
        ArrowTopLeftBottomRight: "arrow-top-left-bottom-right" {
            ac: 0x30,
        },
        ArrowTopRightBottomLeft: "arrow-top-right-bottom-left" {
            ac: 0x30,
        },
        // TODO: Fix 5px, looks like cursor beam
        ArrowUpDown: "arrow-up-down" {
            ac: 0x30,
        } [
            5 {ac: 0x10},
        ],
        // TODO: Needs vertical scale for small sizes to fill the whole box
        Bars: "menu" {
            ac: 0x50,
            // scale_y: 1.0,
            // scale_x: 0.8,
        } [
            // TODO: Fix
            5 {ac: 0x40},
            6 {scale_y: 0.91},
            7 {scale_y: 1.2},
        ],

        Cancel: "cancel" {
            ac: 0x60,
        } [
            5 {ac: 0x30},
        ],
        Check: "check",
        // TODO: Maybe better use bits rotation for symmetric symbols?
        // TODO: Too small on medium sizes
        ChevronLeft: "chevron-left" {
            ac: 0x30,
        },
        ChevronRight: "chevron-right" {
            ac: 0x30,
        },
        ChevronUp: "chevron-up" {
            ac: 0x30,
        },
        ChevronDown: "chevron-down" {
            ac: 0x30,
        },
        Circle: "circle" [
            5 {ac: 0xa0},
        ],
        Close: "close",
        CloseCircle: "close-circle" [
            5 {ac: 0x70},
            6 {ac: 0xc0},
        ],

        Delete: "delete-outline" {
            ac: 0x80,
        } [
            // TODO: Hand-drawn
            5 {ac: 0x3f},
            7 {ac: 0x7f},
            8 {ac: 0x70},
        ],
        DotsHorizontal: "dots-horizontal" [
            5 {ac: 0x50},
            6 {
                ac: 0x80,
                scale_x: 0.9,
                scale_y: 1.2,
            },
        ],
        DotsVertical: "dots-vertical" [
            5 {ac: 0x50},
            6 {
                ac: 0x8f,
                scale_x: 1.2,
                scale_y: 0.9,
            },
        ],

        GreaterThan: "greater-than",

        LessThan: "less-than",

        Minus: "minus" (Remove) {
            ac: 0x00,
        },

        Plus: "plus" (Add) {ac: 0x20} [
            6 {ac: 0x00},
        ],
        PlusMinus: "plus-minus" {
            ac: 0x50,
        } [
            5 {ac: 0x40, scale_y: 0.95},
            6 {ac: 0x30},
            8 {ac: 0x3f},
        ],

        Search: "magnify" {
            ac: 0x60,
        },
        Square: "square" {
            ac: 0x20,
        },
        SquareOutline: "square-outline" {
            ac: 0x20,
        },

        Question: "help" {
            ac: 0x80,
        } [
            5 {ac: 0x60, scale_y: 0.9},
        ],

        UnfoldHorizontal: "unfold-more-horizontal" {
            ac: 0x20,
        } [
            5 {ac: 0x30},
        ],
        UnfoldVertical: "unfold-more-vertical" {
            ac: 0x20,
        } [
            5 {ac: 0x30},
        ],
    }
}
