use crate::console::CtrlEvent;

#[test]
fn every_defined_ctrl_event_maps_from_its_raw_value() {
    assert_eq!(CtrlEvent::from_raw(0), Some(CtrlEvent::C));
    assert_eq!(CtrlEvent::from_raw(1), Some(CtrlEvent::Break));
    assert_eq!(CtrlEvent::from_raw(2), Some(CtrlEvent::Close));
    assert_eq!(CtrlEvent::from_raw(5), Some(CtrlEvent::Logoff));
    assert_eq!(CtrlEvent::from_raw(6), Some(CtrlEvent::Shutdown));
}

#[test]
fn an_undefined_ctrl_event_has_no_mapping() {
    for raw in [3, 4, 7, u32::MAX] {
        assert_eq!(CtrlEvent::from_raw(raw), None);
    }
}

#[cfg(windows)]
mod windows {
    use crate::console::windows::trampoline;
    use crate::console::{
        CtrlEvent, CtrlHandler, add_handler, ignore_ctrl_c, process_ids, restore_ctrl_c,
    };

    struct PassOn;

    impl CtrlHandler for PassOn {
        fn handle(_event: CtrlEvent) -> bool {
            false
        }
    }

    struct TakeCtrlC;

    impl CtrlHandler for TakeCtrlC {
        fn handle(event: CtrlEvent) -> bool {
            event == CtrlEvent::C
        }
    }

    #[test]
    fn the_trampoline_returns_what_the_handler_decides() {
        assert_eq!(trampoline::<TakeCtrlC>(0), 1);
        assert_eq!(trampoline::<TakeCtrlC>(1), 0);
        assert_eq!(trampoline::<PassOn>(0), 0);
        assert_eq!(trampoline::<TakeCtrlC>(3), 0);
    }

    #[test]
    fn a_handler_is_added_after_ctrl_c_is_restored() {
        restore_ctrl_c().expect("restore Ctrl-C");
        add_handler::<PassOn>().expect("add handler");
    }

    #[test]
    fn the_console_list_names_this_process_when_it_has_a_console() {
        let ids = process_ids();
        assert!(ids.is_empty() || ids.contains(&std::process::id()));
    }

    #[test]
    fn ctrl_c_can_be_ignored() {
        ignore_ctrl_c().expect("ignore Ctrl-C");
    }
}
