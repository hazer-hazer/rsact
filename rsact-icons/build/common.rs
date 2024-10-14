use crate::icon_set::icon_set;

// Note: Keep alphabetic order of icon names, please.
// Extended means that symbols aren't universal for all sizes and should be
// added in the future extended icon packs
icon_set! {
    CommonIcon "common" 0x60 [
        6 {ac: 0x60},
        7 {ac: 0x40},
        8 {ac: 0x3f},
        9 {ac: 0x7f},
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
        // TODO: Extended
        // AddBox: "plus-box" {
        //     ac: 0xc0,
        // },
        // TODO: Extended
        // AddCircle: "plus-circle" {
        //     ac: 0xf0,
        // },
        Airplane: "airplane" {
            ac: 0x20,
        },
        AlertBox: "alert-box" {
            ac: 0xa0,
        } [
            6 {ac: 0xd0},
            7 {ac: 0xba},
            9 {ac: 0x40}
        ],
        // TODO: Looks bad
        // AllInclusive: "all-inclusive" {
        //     ac: 0x5a,
        // },
        // TODO
        // AlignLeft: "format-align-left" {
        //     ac: 0xa0,
        // },
        // TODO: Needs scale tuning
        Alpha: "alpha",
        Archive: "archive" {
            ac: 0x7f,
        } [
            5 {ac: 0xb0},
            6 {ac: 0xbf},
            7 {ac: 0x80},
            8 {
                ac: 0xc0,
                scale_y: 1.1
            },
        ],
        // TODO: Extended
        // ArrowAll: "arrow-all" {
        //     ac: 0x60,
        // },
        // TODO: Should not be extended, fix for small sizes
        // ArrowCollapseAll: "arrow-collapse-all" {
        //     ac: 0xa0,
        // },

        Bell: "bell" {
            ac: 0x40,
        } [
            5 {scale_y: 0.9, ac: 0x60},
            7 {scale_y: 0.9},
        ],
        Book: "book" {
            ac: 0x60,
        } [
            5 {ac: 0xa0},
        ],
        Bookmark: "bookmark" {
            ac: 0x80,
        } [
            9 {ac: 0xb0},
        ],
        Bolt: "lightning-bolt" {
            ac: 0x10,
        } [
            // TODO: Looks not the best way
            5 {ac: 0x30, scale_x: 1.1},
            6 {
                ac: 0x40,
                scale_y: 1.2,
            },
        ],
        Brackets: "code-brackets" {
            ac: 0x40,
        } [
            5 {ac: 0x30},
        ],
        Brush: "brush" {
            ac: 0x80,
        },

        // Backward: "skip-backward" {
        //     ac: 0x40,
        // },
        // Bold: "format-bold" {
        //     ac: 0x70,
        // },
        // TODO: Needs tuning
        // CancelBox: "close-box" {
        //     ac: 0xb0,
        // },
        // TODO: Extended, too complex
        // Car: "car" {
        //     ac: 0x3a,
        // },
        // TODO
        // Chip: "memory" {
        //     ac: 0x70,
        // },
        CircleHalf: "circle-half-full",
        CircleOutline: "circle-outline",
        // TODO: Extended, not drawable on small sizes
        // CheckCircle: "check-circle" {
        //     ac: 0xc0,
        // },
        // TODO: Extended, contains small details
        // Clipboard: "clipboard" {
        //     ac: 0xa0,
        //     scale_y: 0.9
        // },
        Clock: "clock-outline" {
            ac: 0x70,
        } [
            5 {ac: 0x30},
            6 {
                ac: 0x3f,
                scale_x: 1.01,
            },
        ],
        // TODO: Needs scale for small sizes to fill
        // TODO: Extended
        // Cloud: "cloud" {
        //     ac: 0x20,
        // },
        // TODO
        // Code: "xml" {
        //     ac: 0x60,
        // },
        Comment: "comment" {
            ac: 0xa0,
        } [
            6 {
                ac: 0xb0,
                scale_x: 0.9,
            },
        ],
        Commit: "source-commit" {
            ac: 0x40,
        } [
            6 {ac: 0x30},
        ],
        // TODO
        // Crop: "crop" {
        //     ac: 0x40,
        // },
        // TODO: Too thin, extended
        // Crosshair: "crosshairs" {
        //     ac: 0x20,
        // },
        Cup: "cup-outline" {
            ac: 0x40,
        } [
            6 {ac: 0x30},
        ],

        // TODO: Needs scale to fill small sizes + ac selection
        Delete: "delete-outline" {
            ac: 0x80,
        } [
            7 {ac: 0x7f},
            8 {ac: 0x70},
        ],
        Diamond: "cards-diamond",
        // TODO: Fix 5px
        DotOutline: "adjust" [
            5 {ac: 0x40},
        ],
        // TODO: Extended
        // Drag: "drag",
        Droplet: "water" {
            ac: 0x20,
        } [
            9 {ac: 0x10},
        ],

        Envelope: "email" {
            ac: 0x80,
        } [
            5 {ac: 0x90},
        ],
        Eraser: "eraser",
        Exclamation: "exclamation" {
            ac: 0x20,
        } [
            5 {ac: 0x30, scale_y: 0.95},
            7 {ac: 0x40},
        ],
        Eye: "eye" {
            ac: 0x80,
        } [
            5 {ac: 0x80},
            6 {ac: 0x70},
        ],
        EyeClosed: "eye-closed",
        Equal: "equal" {
            ac: 0x40,
        } [
            6 {ac: 0x3f},
        ],
        // TODO: Bad on 6px
        // Eject: "eject" {
        //     ac: 0x80,
        // },
        // Expand: "crop-free" {
        //     ac: 0x50,
        // },

        // TODO: Hmmm.... I'm not a fan of what fan looks like on 6px
        // Fan: "fan" {
        //     ac: 0xc0,
        // },
        File: "file" {
            ac: 0x80,
        } [
            5 {ac: 0x40}
        ],
        Filter: "filter",
        // TODO: Needs scale for small sizes
        // FilterList: "filter-variant" {
        //     ac: 0x40,
        // },
        // Fish: "fish" {
        //     ac: 0x50,
        // },
        Flag: "flag" {
            ac: 0x20,
        } [
            5 {ac: 0x40},
        ],
        FlaskOutline: "flask-empty-outline" {
            ac: 0x20,
        },
        // TODO: Extended
        // Flask: "flask-empty" {
        //     ac: 0x10,
        // },
        Folder: "folder" {
            ac: 0x00,
        } [
            5 {ac: 0x80},
        ],
        // TODO: Extended, too complex
        // FolderOpen: "folder-open" {
        //     ac: 0x60,
        // },
        // TODO: Needs scale for small sizes
        // Fullscreen: "fullscreen" {
        //     ac: 0x10,
        // },
        Function: "function" {
            ac: 0x50,
        } [
            6 {
                ac: 0x20,
                scale_x: 1.1
            }
        ],

        // TODO: Extended
        // Glasses: "glasses" {
        //     ac: 0x20,
        // },

        // TODO: Extended
        // Hashtag: "pound" {
        //     ac: 0x20,
        // },
        // TODO
        // Headphones: "headphones" {
        //     ac: 0x80,
        // },
        Heart: "heart" {
            ac: 0x50,
        } [
            5 {ac: 0xd0},
            6 {ac: 0xc0},
            7 {ac: 0xa0},
            9 {ac: 0x50},
        ],
        Home: "home" {
            ac: 0x50,
        },
        // TODO: Remove?
        // HomeRoof: "home-roof" {
        //     ac: 0x30,
        // },
        Hourglass: "timer-sand-empty" {
            ac: 0x30,
        } [
            7 {ac: 0x7a},
        ],

        // TODO
        // Indent: "format-indent-increase" {
        //     ac: 0x50,
        // },

        Knob: "knob" {
            ac: 0xc0,
        },

        // LessThanOrEqual: "less-than-or-equal",
        Lightbulb: "lightbulb" {
            ac: 0x20,
        },
        Link: "link" {
            ac: 0x30,
        } [
            5 {ac: 0x40},
            8 {ac: 0x60},
        ],
        // TODO: No simple List icon in material design, wtf. either bold either complex
        // List: "format-list-bulleted-square" {
        //     ac: 0x20,
        //     // scale_y: 1.2,
        // },
        // TODO: Basic icon, problems on small sizes
        // Lock: "lock",
        // TODO: Review on medium sizes
        Login: "login" {
            ac: 0x30,
        },
        Logout: "logout" {
            ac: 0x20,
        },

        MapMarker: "map-marker" {
            ac: 0x80,
        } [
            5 {ac: 0x40},
        ],
        Magnet: "magnet" {
            ac: 0x40,
        } [
            6 {
                ac: 0xb0,
                scale_y: 1.05,
            },
        ],
        Music: "music" [
            5 {ac: 0x40},
        ],
        // TODO
        // Map: "map-outline" {
        //     ac: 0x70,
        // },

        Navigation: "navigation" [
            5 {ac: 0x50},
        ],
        Note: "note" {
            ac: 0x7a,
        },

        // TODO: Too complex, extended
        // Paw: "paw" {
        //     ac: 0x90,
        //     scale_y: 1.1,
        // },
        // TODO
        // PowerPlug: "power-plug-outline" {
        //     ac: 0x30,
        // },
        Pause: "pause" {
            ac: 0x40,
        } [
            5 {ac: 0x80},
        ],
        Pencil: "pencil" {
            ac: 0x50,
        } [
            8 {ac: 0x70},
        ],
        Percent: "percent" {
            ac: 0x50,
        },
        Phone: "phone" {
            ac: 0x50,
        },
        Pin: "pin" {
            ac: 0x40,
        } [
            // TODO: Hand-drawn
            // 5 {ac: 0x60, scale_y: 0.9},
            8 {ac: 0x30},
        ],
        Play: "play" {
            ac: 0x10,
        },
        Poll: "poll",
        Power: "power-standby" {
            ac: 0x30,
        },

        // TODO: Too complex, extended
        // Radiation: "radioactive-circle" {
        //     ac: 0x90
        // },
        RemoveCircle: "minus-circle" {
            ac: 0xd0,
        },
        // TODO: Transform
        // Rotate: "rotate-3d-variant" {
        //     ac: 0x40,
        // },
        RotateLeft: "restore" {
            ac: 0x50,
        },
        RotateRight: "reload" {
            ac: 0x50,
        },
        Rhombus: "rhombus",
        RhombusOutline: "rhombus-outline",
        // TODO: Too complex for small sizes
        // Refresh: "refresh" {
        //     ac: 0x40,
        // },
        // TODO: Extended
        // Rss: "rss" {
        //     ac: 0x40,
        // },


        // TODO: Fix 6px
        // SawtoothWave: "sawtooth-wave" {
        //     ac: 0x40,
        // },
        // TODO: Not drawable on small sizes, move to extended list
        // Settings: "cog" {
        //     ac: 0xb0,
        // },
        Send: "send" {
            ac: 0x60,
        } [
            6 {ac: 0x80},
        ],
        // TODO: Extended
        // Shapes: "shape" {
        //     ac: 0x90,
        // },
        Share: "share" {
            ac: 0x40,
        } [
            8 {ac: 0x20},
        ],
        ShareNodes: "share-variant" {
            ac: 0x40,
        },
        // TODO: Extended
        // Shield: "shield" {
        //     ac: 0x80,
        // },
        // TODO: Extended
        // Shuffle: "shuffle-variant" {
        //     ac: 0x40,
        // },
        // TODO
        // Signal: "signal-cellular-3" {
        //     ac: 0x80,
        // },
        SineWave: "sine-wave" {
            ac: 0x30,
        },
        // TODO: Extended
        // Sitemap: "sitemap" {
        //     ac: 0x50,
        // },
        // TODO: Needs scale on small variants
        // Sort: "sort-variant" {
        //     ac: 0x30,
        // },
        Speaker: "volume-low" {
            ac: 0x30,
        },
        // SquareOpacity: "square-opacity" {
        //     ac: 0x90,
        // },
        SquareRounded: "square-rounded" {
            ac: 0x60,
        },
        SquareRoundedOutline: "square-rounded-outline" {
            ac: 0x60,
        },
        SquareWave: "square-wave" {
            ac: 0x20,
        },
        // TODO
        // Stairs: "stairs" {
        //     ac: 0x40,
        //     scale_x: 1.1,
        // },
        // TODO: Extended
        // Star: "star" {
        //     ac: 0xa0,
        // },
        StepBackward: "step-backward" {
            ac: 0x80
        },
        StepForward: "step-forward" {
            ac: 0x80,
        },

        Tag: "tag" {
            ac: 0x80,
        },
        Terminal: "console-line" {
            ac: 0x20,
        } [
            6 {ac: 0x50},
        ],
        Thermometer: "thermometer" {
            ac: 0x50,
        } [
            7 {ac: 0x30},
        ],
        // TODO
        // Toggle: "toggle-switch-outline" {
        //     ac: 0x40,
        // },
        // Tree: "pine-tree" {
        //     ac: 0x90,
        // },
        TriangleWave: "triangle-wave" [
            5 {ac: 0x20}
        ],

        // TODO: Too complex, extended
        // QuoteLeft: "format-quote-open" {
        //     ac: 0x80,
        // },

        // TODO: Extended cause too complex
        // Undo: "undo" {
        //     ac: 0x00,
        // },

        // TODO: Too complex, extended
        // Video: "video" {
        //     ac: 0xa0,
        // },
        VolumeHigh: "volume-high" {
            ac: 0x60,
        },

        Wifi: "wifi" {
            ac: 0x70,
        } [
            5 {ac: 0x7f, scale_y: 1.2},
        ],
    }
}
