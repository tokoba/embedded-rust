#!/bin/bash
echo "[サイズチェック/bloatチェック]"
echo "-------------------------------------"
echo "[target] blinky, [feature] singletask"
cargo size --release --bin blinky && cargo size --release --bin blinky -- -A && cargo bloat --release --bin blinky --crates && cargo bloat --release --bin blinky -n 10
echo "-------------------------------------"
echo "[target] blinky, [feature] multitask"
cargo size --release --bin blinky --features multitask && cargo size --release --bin blinky --features multitask -- -A && cargo bloat --release --bin blinky --features multitask --crates && cargo bloat --release --bin blinky --features multitask -n 10
echo "-------------------------------------"
echo "[target] button_exti"
cargo size --release --bin button_exti && cargo size --release --bin button_exti -- -A && cargo bloat --release --bin button_exti --crates && cargo bloat --release --bin button_exti -n 10
echo "-------------------------------------"
echo "[target] button_led"
cargo size --release --bin button_led && cargo size --release --bin button_led -- -A && cargo bloat --release --bin button_led --crates && cargo bloat --release --bin button_led -n 10
echo "-------------------------------------"
echo "[target] eth"
cargo size --release --bin eth && cargo size --release --bin eth -- -A && cargo bloat --release --bin eth --crates && cargo bloat --release --bin eth -n 10
echo "-------------------------------------"
echo "[target] usb_serial"
cargo size --release --bin usb_serial && cargo size --release --bin usb_serial -- -A && cargo bloat --release --bin usb_serial --crates && cargo bloat --release --bin usb_serial -n 10
echo "-------------------------------------"
echo "[target] eth_usb_serial"
cargo size --release --bin eth_usb_serial && cargo size --release --bin eth_usb_serial -- -A && cargo bloat --release --bin eth_usb_serial --crates && cargo bloat --release --bin eth_usb_serial -n 10
echo "-------------------------------------"
echo "[target] flash_control"
cargo size --release --bin flash_control && cargo size --release --bin flash_control -- -A && cargo bloat --release --bin flash_control --crates && cargo bloat --release --bin flash_control -n 10
