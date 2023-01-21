// This module is for testing and demonstrating alternative approaches
// that didn't make the cut for various reasons into the library
// Having this nested sub folder under tests along with its own main.rs
// is the suggested approach from the cargo docs for having a nested folder
// under tests.
// See: https://doc.rust-lang.org/stable/cargo/guide/project-layout.html

#![cfg(feature = "experiments")]

// Uncomment sections within this module
// as a refresher on various kinds of casting that rust can't do out of the box
// This has been left here for reference purposes and as part of the motivational
// documentation for why the upcasting module was created
mod casting_limitations;

mod interior_guardcell_1_pass;

// mod interior_guardcell_2_fail; // Uncomment to see errors

mod interior_guardcell_3_pass;