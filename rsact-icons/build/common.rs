use crate::icon_set::icon_set;

// Note: Keep alphabetic order of icon names, please.
// Extended means that symbols aren't universal for all sizes and should be
// added in the future extended icon packs
icon_set! {
    CommonIcon 0x60 [
        6 {alpha_cutoff: 0x60},
        7 {alpha_cutoff: 0x40},
        8 {alpha_cutoff: 0x3f},
        9 {alpha_cutoff: 0x7f},
        10,
        11,
        12,
        13,
        14,
        15,
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
        // atom: Atom,
        Account: "account",
        AccountBox: "account-box",
        AccountCircle: "account-circle",
        Add: "plus" [
            6 {
                alpha_cutoff: 0x00,
            },
        ],
        // TODO: Extended
        // AddBox: "plus-box" {
        //     alpha_cutoff: 0xc0,
        // },
        // TODO: Extended
        // AddCircle: "plus-circle" {
        //     alpha_cutoff: 0xf0,
        // },
        Adjust: "adjust",
        AirPlane: "airplane" {
            alpha_cutoff: 0x20,
        },
        AlertBox: "alert-box" {
            alpha_cutoff: 0xa0,
        } [
            6 {
                alpha_cutoff: 0xd0,
            },
            7 {
                alpha_cutoff: 0xba,
            },
            9 {
                alpha_cutoff: 0x40,
            }
        ],
        // TODO: Looks bad
        // AllInclusive: "all-inclusive" {
        //     alpha_cutoff: 0x5a,
        // },
        // TODO
        // AlignLeft: "format-align-left" {
        //     alpha_cutoff: 0xa0,
        // },
        // TODO: Needs scale tuning
        Alpha: "alpha",
        Archive: "archive" {
            alpha_cutoff: 0x7f,
        },
        // TODO: Extended
        // ArrowAll: "arrow-all" {
        //     alpha_cutoff: 0x60,
        // },
        ArrowExpand: "arrow-expand" {
            alpha_cutoff: 0x30,
        },
        ArrowExpandAll: "arrow-expand-all" {
            alpha_cutoff: 0x30,
        },
        // TODO: Review on 6px
        ArrowBottomLeft: "arrow-bottom-left" {
            alpha_cutoff: 0x40,
        },
        ArrowBottomRight: "arrow-bottom-right" {
            alpha_cutoff: 0x60,
        },
        // TODO: Should not be extended, fix for small sizes
        // ArrowCollapseAll: "arrow-collapse-all" {
        //     alpha_cutoff: 0xa0,
        // },
        ArrowTopLeft: "arrow-top-left" {
            alpha_cutoff: 0x40,
        },
        ArrowTopRight: "arrow-top-right" {
            alpha_cutoff: 0x40,
        },
        // TODO: alpha_cutoff + scale on small sizes
        ArrowLeft: "arrow-left" {
            alpha_cutoff: 0x20,
        },
        ArrowRight: "arrow-right" {
            alpha_cutoff: 0x20,
        },
        ArrowTopLeftBottomRight: "arrow-top-left-bottom-right" {
            alpha_cutoff: 0x30,
        },
        ArrowTopRightBottomLeft: "arrow-top-right-bottom-left" {
            alpha_cutoff: 0x30,
        },
        ArrowUpDown: "arrow-up-down" {
            alpha_cutoff: 0x30,
        },

        // Backward: "skip-backward" {
        //     alpha_cutoff: 0x40,
        // },
        // TODO: Needs vertical scale for small sizes to fill the whole box
        Bars: "menu" {
            alpha_cutoff: 0x50,
            // scale_y: 1.0,
            // scale_x: 0.8,
        },
        Bell: "bell" {
            alpha_cutoff: 0x40,
        },
        Book: "book" {
            alpha_cutoff: 0x60,
        },
        Bookmark: "bookmark" {
            alpha_cutoff: 0x80,
        },
        // Bold: "format-bold" {
        //     alpha_cutoff: 0x70,
        // },
        Bolt: "lightning-bolt" {
            alpha_cutoff: 0x10,
        },
        Brackets: "code-brackets" {
            alpha_cutoff: 0x40,
        },
        Brush: "brush" {
            alpha_cutoff: 0x80,
        },

        Cancel: "cancel" {
            alpha_cutoff: 0x60,
        },
        // TODO: Needs tuning
        // CancelBox: "close-box" {
        //     alpha_cutoff: 0xb0,
        // },
        // TODO: Extended, too complex
        // Car: "car" {
        //     alpha_cutoff: 0x3a,
        // },
        Check: "check",
        // TODO: Maybe better use bits rotation for symmetric symbols?
        // TODO: Too small on medium sizes
        ChevronLeft: "chevron-left" {
            alpha_cutoff: 0x30,
        },
        ChevronRight: "chevron-right" {
            alpha_cutoff: 0x30,
        },
        ChevronUp: "chevron-up" {
            alpha_cutoff: 0x30,
        },
        ChevronDown: "chevron-down" {
            alpha_cutoff: 0x30,
        },
        // TODO
        // Chip: "memory" {
        //     alpha_cutoff: 0x70,
        // },
        Circle: "circle",
        CircleHalf: "circle-half-full",
        CircleOutline: "circle-outline",
        // TODO: Extended, not drawable on small sizes
        // CheckCircle: "check-circle" {
        //     alpha_cutoff: 0xc0,
        // },
        // TODO: Extended, contains small details
        // Clipboard: "clipboard" {
        //     alpha_cutoff: 0xa0,
        //     scale_y: 0.9
        // },
        Clock: "clock-outline" {
            alpha_cutoff: 0x70,
        },
        // TODO: Needs scale for small sizes to fill
        Close: "close",
        // TODO: Extended
        CloseCircle: "close-circle",
        // TODO: Extended
        // Cloud: "cloud" {
        //     alpha_cutoff: 0x20,
        // },
        // TODO
        // Code: "xml" {
        //     alpha_cutoff: 0x60,
        // },
        Comment: "comment" {
            alpha_cutoff: 0xa0,
        },
        Commit: "source-commit" {
            alpha_cutoff: 0x40,
        },
        // TODO
        // Crop: "crop" {
        //     alpha_cutoff: 0x40,
        // },
        // TODO: Too thin, extended
        // Crosshair: "crosshairs" {
        //     alpha_cutoff: 0x20,
        // },
        Cup: "cup-outline" {
            alpha_cutoff: 0x40,
        },

        // TODO: Needs scale to fill small sizes + alpha_cutoff selection
        Delete: "delete-outline" {
            alpha_cutoff: 0x80,
        },
        Diamond: "cards-diamond",
        DotsHorizontal: "dots-horizontal",
        DotsVertical: "dots-vertical",
        // TODO: Extended
        // Drag: "drag",
        Droplet: "water" {
            alpha_cutoff: 0x20,
        },

        // TODO: Bad on 6px
        // Eject: "eject" {
        //     alpha_cutoff: 0x80,
        // },
        Envelope: "email" {
            alpha_cutoff: 0x80,
        },
        Eraser: "eraser",
        Exclamation: "exclamation" {
            alpha_cutoff: 0x20,
        },
        // Expand: "crop-free" {
        //     alpha_cutoff: 0x50,
        // },
        Eye: "eye" {
            alpha_cutoff: 0x80,
        },
        EyeClosed: "eye-closed",
        Equal: "equal" {
            alpha_cutoff: 0x40,
        },

        // TODO: Hmmm.... I'm not a fan of what fan looks like on 6px
        // Fan: "fan" {
        //     alpha_cutoff: 0xc0,
        // },
        File: "file" {
            alpha_cutoff: 0x80,
        },
        // TODO: Needs scale for small sizes
        // FilterList: "filter-variant" {
        //     alpha_cutoff: 0x40,
        // },
        Filter: "filter" {
            // alpha_cutoff: 0x40
        },
        // Fish: "fish" {
        //     alpha_cutoff: 0x50,
        // },
        Flag: "flag" {
            alpha_cutoff: 0x20,
        },
        // TODO: Extended
        // Flask: "flask-empty" {
        //     alpha_cutoff: 0x10,
        // },
        FlaskOutline: "flask-empty-outline" {
            alpha_cutoff: 0x20,
        },
        Folder: "folder" {
            alpha_cutoff: 0x00,
        },
        // TODO: Extended, too complex
        // FolderOpen: "folder-open" {
        //     alpha_cutoff: 0x60,
        // },
        // TODO: Needs scale for small sizes
        // Fullscreen: "fullscreen" {
        //     alpha_cutoff: 0x10,
        // },
        Function: "function" {
            alpha_cutoff: 0x50,
        },

        // TODO: Extended
        // Glasses: "glasses" {
        //     alpha_cutoff: 0x20,
        // },
        GreaterThan: "greater-than",

        // TODO: Extended
        // Hashtag: "pound" {
        //     alpha_cutoff: 0x20,
        // },
        Heart: "heart" {
            alpha_cutoff: 0x50,
        },
        // TODO
        // Headphones: "headphones" {
        //     alpha_cutoff: 0x80,
        // },
        Home: "home" {
            alpha_cutoff: 0x50,
        },
        // TODO: Remove?
        // HomeRoof: "home-roof" {
        //     alpha_cutoff: 0x30,
        // },
        Hourglass: "timer-sand-empty" {
            alpha_cutoff: 0x30,
        },

        // TODO
        // Indent: "format-indent-increase" {
        //     alpha_cutoff: 0x50,
        // },

        Knob: "knob" {
            alpha_cutoff: 0xc0,
        },

        LessThan: "less-than",
        // LessThanOrEqual: "less-than-or-equal",
        // TODO: No simple List icon in material design, wtf. either bold either complex
        // List: "format-list-bulleted-square" {
        //     alpha_cutoff: 0x20,
        //     // scale_y: 1.2,
        // },
        Lightbulb: "lightbulb" {
            alpha_cutoff: 0x20,
        },
        Link: "link" {
            alpha_cutoff: 0x30,
        },
        // TODO: Basic icon, problems on small sizes
        // Lock: "lock",
        // TODO: Review on medium sizes
        Login: "login" {
            alpha_cutoff: 0x30,
        },
        Logout: "logout" {
            alpha_cutoff: 0x20,
        },

        Magnet: "magnet" {
            alpha_cutoff: 0x40,
        },
        // TODO
        // Map: "map-outline" {
        //     alpha_cutoff: 0x70,
        // },
        MapMarker: "map-marker" {
            alpha_cutoff: 0x80,
        },
        Music: "music",

        Navigation: "navigation",
        Note: "note" {
            alpha_cutoff: 0x7a,
        },

        Pause: "pause" {
            alpha_cutoff: 0x40,
        },
        // TODO: Too complex, extended
        // Paw: "paw" {
        //     alpha_cutoff: 0x90,
        //     scale_y: 1.1,
        // },
        Pencil: "pencil" {
            alpha_cutoff: 0x50,
        },
        Percent: "percent" {
            alpha_cutoff: 0x50,
        },
        Phone: "phone" {
            alpha_cutoff: 0x50,
        },
        Pin: "pin" {
            alpha_cutoff: 0x40,
        },
        Play: "play" {
            alpha_cutoff: 0x10,
        },
        PlusMinus: "plus-minus" {
            alpha_cutoff: 0x50,
        },
        Poll: "poll",
        Power: "power-standby" {
            alpha_cutoff: 0x30,
        },
        // TODO
        // PowerPlug: "power-plug-outline" {
        //     alpha_cutoff: 0x30,
        // },

        // TODO: Too complex, extended
        // Radiation: "radioactive-circle" {
        //     alpha_cutoff: 0x90
        // },
        Remove: "minus" {
            alpha_cutoff: 0x00,
        },
        RemoveCircle: "minus-circle" {
            alpha_cutoff: 0xd0,
        },
        // TODO: Transform
        // Rotate: "rotate-3d-variant" {
        //     alpha_cutoff: 0x40,
        // },
        RotateLeft: "restore" {
            alpha_cutoff: 0x50,
        },
        RotateRight: "reload" {
            alpha_cutoff: 0x50,
        },
        Rhombus: "rhombus",
        RhombusOutline: "rhombus-outline",
        // TODO: Too complex for small sizes
        // Refresh: "refresh" {
        //     alpha_cutoff: 0x40,
        // },
        // TODO: Extended
        // Rss: "rss" {
        //     alpha_cutoff: 0x40,
        // },


        // TODO: Fix 6px
        // SawtoothWave: "sawtooth-wave" {
        //     alpha_cutoff: 0x40,
        // },
        Search: "magnify" {
            alpha_cutoff: 0x60,
        },
        // TODO: Not drawable on small sizes, move to extended list
        // Settings: "cog" {
        //     alpha_cutoff: 0xb0,
        // },
        Send: "send" {
            alpha_cutoff: 0x60,
        },
        // TODO: Extended
        // Shapes: "shape" {
        //     alpha_cutoff: 0x90,
        // },
        Share: "share" {
            alpha_cutoff: 0x40,
        },
        ShareNodes: "share-variant" {
            alpha_cutoff: 0x40,
        },
        // TODO: Extended
        // Shield: "shield" {
        //     alpha_cutoff: 0x80,
        // },
        // TODO: Extended
        // Shuffle: "shuffle-variant" {
        //     alpha_cutoff: 0x40,
        // },
        // TODO
        // Signal: "signal-cellular-3" {
        //     alpha_cutoff: 0x80,
        // },
        SineWave: "sine-wave" {
            alpha_cutoff: 0x30,
        },
        // TODO: Extended
        // Sitemap: "sitemap" {
        //     alpha_cutoff: 0x50,
        // },
        // TODO: Needs scale on small variants
        // Sort: "sort-variant" {
        //     alpha_cutoff: 0x30,
        // },
        Speaker: "volume-low" {
            alpha_cutoff: 0x30,
        },
        Square: "square" {
            alpha_cutoff: 0x20,
        },
        // SquareOpacity: "square-opacity" {
        //     alpha_cutoff: 0x90,
        // },
        SquareOutline: "square-outline" {
            alpha_cutoff: 0x20,
        },
        SquareRounded: "square-rounded" {
            alpha_cutoff: 0x60,
        },
        SquareRoundedOutline: "square-rounded-outline" {
            alpha_cutoff: 0x60,
        },
        SquareWave: "square-wave" {
            alpha_cutoff: 0x20,
        },
        // TODO
        // Stairs: "stairs" {
        //     alpha_cutoff: 0x40,
        //     scale_x: 1.1,
        // },
        // TODO: Extended
        // Star: "star" {
        //     alpha_cutoff: 0xa0,
        // },
        StepBackward: "step-backward" {
            alpha_cutoff: 0x80
        },
        StepForward: "step-forward" {
            alpha_cutoff: 0x80,
        },

        Tag: "tag" {
            alpha_cutoff: 0x80,
        },
        Terminal: "console-line" {
            alpha_cutoff: 0x20,
        },
        Thermometer: "thermometer" {
            alpha_cutoff: 0x50,
        },
        // TODO
        // Toggle: "toggle-switch-outline" {
        //     alpha_cutoff: 0x40,
        // },
        // Tree: "pine-tree" {
        //     alpha_cutoff: 0x90,
        // },
        TriangleWave: "triangle-wave",

        Question: "help" {
            alpha_cutoff: 0x80,
        },
        // TODO: Too complex, extended
        // QuoteLeft: "format-quote-open" {
        //     alpha_cutoff: 0x80,
        // },

        // TODO: Extended cause too complex
        // Undo: "undo" {
        //     alpha_cutoff: 0x00,
        // },
        UnfoldHorizontal: "unfold-more-horizontal" {
            alpha_cutoff: 0x20,
        },
        UnfoldVertical: "unfold-more-vertical" {
            alpha_cutoff: 0x20,
        },

        // TODO: Too complex, extended
        // Video: "video" {
        //     alpha_cutoff: 0xa0,
        // },
        VolumeHigh: "volume-high" {
            alpha_cutoff: 0x60,
        },

        Wifi: "wifi" {
            alpha_cutoff: 0x70,
        },
    }
}
