#![no_std]
#![no_main]

use core::option::Option::Some;
use defmt::info;
use defmt_rtt as _; // global logger
use embassy_executor::Spawner;
use embassy_stm32::gpio::OutputType;
use embassy_stm32::interrupt;
use embassy_stm32::pac;
use embassy_stm32::rcc::*;
use embassy_stm32::time::hz;
use embassy_stm32::timer::low_level::Timer as LLTimer;
use embassy_stm32::timer::low_level::*;
use embassy_stm32::timer::simple_pwm::PwmPin;
use embassy_stm32::timer::Channel;
use embassy_stm32::Config;
use panic_probe as _;

const DDS_SINE_DATA: [u8; 256] = [
    0x80, 0x83, 0x86, 0x89, 0x8c, 0x8f, 0x92, 0x95, 0x98, 0x9c, 0x9f, 0xa2, 0xa5, 0xa8, 0xab, 0xae, 0xb0, 0xb3, 0xb6,
    0xb9, 0xbc, 0xbf, 0xc1, 0xc4, 0xc7, 0xc9, 0xcc, 0xce, 0xd1, 0xd3, 0xd5, 0xd8, 0xda, 0xdc, 0xde, 0xe0, 0xe2, 0xe4,
    0xe6, 0xe8, 0xea, 0xec, 0xed, 0xef, 0xf0, 0xf2, 0xf3, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfc, 0xfd,
    0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfc, 0xfc, 0xfb,
    0xfa, 0xf9, 0xf8, 0xf7, 0xf6, 0xf5, 0xf3, 0xf2, 0xf0, 0xef, 0xed, 0xec, 0xea, 0xe8, 0xe6, 0xe4, 0xe2, 0xe0, 0xde,
    0xdc, 0xda, 0xd8, 0xd5, 0xd3, 0xd1, 0xce, 0xcc, 0xc9, 0xc7, 0xc4, 0xc1, 0xbf, 0xbc, 0xb9, 0xb6, 0xb3, 0xb0, 0xae,
    0xab, 0xa8, 0xa5, 0xa2, 0x9f, 0x9c, 0x98, 0x95, 0x92, 0x8f, 0x8c, 0x89, 0x86, 0x83, 0x80, 0x7c, 0x79, 0x76, 0x73,
    0x70, 0x6d, 0x6a, 0x67, 0x63, 0x60, 0x5d, 0x5a, 0x57, 0x54, 0x51, 0x4f, 0x4c, 0x49, 0x46, 0x43, 0x40, 0x3e, 0x3b,
    0x38, 0x36, 0x33, 0x31, 0x2e, 0x2c, 0x2a, 0x27, 0x25, 0x23, 0x21, 0x1f, 0x1d, 0x1b, 0x19, 0x17, 0x15, 0x13, 0x12,
    0x10, 0x0f, 0x0d, 0x0c, 0x0a, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x03, 0x02, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x02, 0x03, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
    0x0a, 0x0c, 0x0d, 0x0f, 0x10, 0x12, 0x13, 0x15, 0x17, 0x19, 0x1b, 0x1d, 0x1f, 0x21, 0x23, 0x25, 0x27, 0x2a, 0x2c,
    0x2e, 0x31, 0x33, 0x36, 0x38, 0x3b, 0x3e, 0x40, 0x43, 0x46, 0x49, 0x4c, 0x4f, 0x51, 0x54, 0x57, 0x5a, 0x5d, 0x60,
    0x63, 0x67, 0x6a, 0x6d, 0x70, 0x73, 0x76, 0x79, 0x7c,
];

// frequency: 15625/(256/(DDS_INCR/2**24)) = 999,99999Hz
static mut DDS_INCR: u32 = 0x10624DD2;

// fractional phase accumulator
static mut DDS_AKKU: u32 = 0x00000000;

#[interrupt]
fn TIM2() {
    unsafe {
        // get next value of DDS
        DDS_AKKU = DDS_AKKU.wrapping_add(DDS_INCR);
        let value = (DDS_SINE_DATA[(DDS_AKKU >> 24) as usize] as u16) << 3;

        // set new output compare value
        pac::TIM2.ccr(2).modify(|w| w.set_ccr(value));

        // reset interrupt flag
        pac::TIM2.sr().modify(|r| r.set_uif(false));
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello World!");

    // configure for 32MHz (HSI16 * 6 / 3)
    let mut config = Config::default();
    config.rcc.sys = Sysclk::PLL1_R;
    config.rcc.hsi = true;
    config.rcc.pll = Some(Pll {
        source: PllSource::HSI,
        div: PllDiv::DIV3,
        mul: PllMul::MUL6,
    });

    let p = embassy_stm32::init(config);

    // setup PWM pin in AF mode
    let _ch3 = PwmPin::new_ch3(p.PA2, OutputType::PushPull);

    // initialize timer
    let timer = LLTimer::new(p.TIM2);

    // set counting mode
    timer.set_counting_mode(CountingMode::EdgeAlignedUp);

    // set pwm sample frequency
    timer.set_frequency(hz(15625));

    // enable outputs
    timer.enable_outputs();

    // start timer
    timer.start();

    // set output compare mode
    timer.set_output_compare_mode(Channel::Ch3, OutputCompareMode::PwmMode1);

    // set output compare preload
    timer.set_output_compare_preload(Channel::Ch3, true);

    // set output polarity
    timer.set_output_polarity(Channel::Ch3, OutputPolarity::ActiveHigh);

    // set compare value
    timer.set_compare_value(Channel::Ch3, timer.get_max_compare_value() / 2);

    // enable pwm channel
    timer.enable_channel(Channel::Ch3, true);

    // enable timer interrupts
    timer.enable_update_interrupt(true);
    unsafe { cortex_m::peripheral::NVIC::unmask(interrupt::TIM2) };

    async {
        loop {
            embassy_time::Timer::after_millis(5000).await;
        }
    }
    .await;
}
