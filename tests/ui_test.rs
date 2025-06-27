// Simple UI test
use std::io;

fn main() -> io::Result<()> {
    println!("Testing UI functionality...");
    
    // Test basic ANSI colors
    println!("\x1b[31mRed text\x1b[0m");
    println!("\x1b[32mGreen text\x1b[0m");
    println!("\x1b[34mBlue text\x1b[0m");
    
    // Test box drawing characters
    println!("┌─────────────────┐");
    println!("│ Simple box test │");
    println!("└─────────────────┘");
    
    println!("╔═══════════════════╗");
    println!("║ Double line box   ║");
    println!("╚═══════════════════╝");
    
    println!("╭───────────────────╮");
    println!("│ Rounded box test  │");
    println!("╰───────────────────╯");
    
    println!("UI test completed successfully!");
    Ok(())
}