#!/usr/bin/env python3
"""
古代浑仪机械传动误差仿真 - 传感器模拟器
模拟宋代浑仪各轴传感器每分钟上报数据
"""

import json
import math
import random
import time
import datetime
import requests
import argparse
import paho.mqtt.client as mqtt
from typing import Dict, Any, Optional

DEG_TO_RAD = math.pi / 180.0
DEG_TO_ARCMIN = 60.0


class HunyiSensorSimulator:
    def __init__(
        self,
        device_id: str = "HUNYI-001",
        api_url: str = "http://localhost:8080",
        mqtt_broker: Optional[str] = None,
        mqtt_topic: str = "hunyi/sensor",
        initial_wear: tuple = (0.1, 0.08, 0.09),
        wear_rate: float = 1.0,
        backlash_multiplier: float = 1.0,
        noise_multiplier: float = 1.0,
    ):
        self.device_id = device_id
        self.api_url = api_url
        self.endpoint = f"{api_url}/api/v1/sensor/ingest"
        self.mqtt_broker = mqtt_broker
        self.mqtt_topic = mqtt_topic

        self.azimuth_angle = 0.0
        self.elevation_angle = 45.0
        self.equatorial_angle = 0.0

        self.azimuth_speed = 0.15
        self.elevation_speed = 0.02
        self.equatorial_speed = 0.25

        self.gear_wear_1 = max(0.0, min(0.99, initial_wear[0]))
        self.gear_wear_2 = max(0.0, min(0.99, initial_wear[1]))
        self.gear_wear_3 = max(0.0, min(0.99, initial_wear[2]))

        self.base_temperature = 22.0
        self.base_humidity = 55.0

        self.wear_rate = wear_rate
        self.backlash_multiplier = backlash_multiplier
        self.noise_multiplier = noise_multiplier

        self.tick_count = 0

        self.mqtt_client = None
        if mqtt_broker:
            self._init_mqtt()

    def _init_mqtt(self):
        try:
            self.mqtt_client = mqtt.Client(client_id=f"sim-{self.device_id}", protocol=mqtt.MQTTv5)
            host, port = self.mqtt_broker.split(':') if ':' in self.mqtt_broker else (self.mqtt_broker, 1883)
            self.mqtt_client.connect(host, int(port), keepalive=60)
            self.mqtt_client.loop_start()
            print(f"[MQTT] Connected to broker {self.mqtt_broker}, topic={self.mqtt_topic}")
        except Exception as e:
            print(f"[MQTT WARN] Failed to connect: {e}, will fall back to HTTP")
            self.mqtt_client = None

    def _generate_angles(self) -> tuple:
        noise = self.noise_multiplier
        self.azimuth_angle = (self.azimuth_angle + self.azimuth_speed + random.uniform(-0.02, 0.02) * noise) % 360.0
        self.elevation_angle = max(
            5.0,
            min(85.0, self.elevation_angle + random.uniform(-0.05, 0.05) * noise + self.elevation_speed * math.sin(self.tick_count * 0.01))
        )
        self.equatorial_angle = (self.equatorial_angle + self.equatorial_speed + random.uniform(-0.03, 0.03) * noise) % 360.0

        return self.azimuth_angle, self.elevation_angle, self.equatorial_angle

    def _generate_gear_meshing_errors(self) -> tuple:
        base_errors = [0.12, 0.10, 0.11]
        wears = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3]
        errors = []
        for i in range(3):
            base = base_errors[i] * self.backlash_multiplier
            wear_multiplier = 1.0 + wears[i] * 3.0
            oscillation = 0.05 * math.sin(self.tick_count * 0.1 + i * 2.0) * self.noise_multiplier
            noise = random.gauss(0, 0.02) * self.noise_multiplier
            error = base * wear_multiplier + oscillation + noise
            errors.append(max(0.0, error))
        return tuple(errors)

    def _generate_bearing_clearances(self) -> tuple:
        bases = [0.15, 0.12, 0.14]
        clearances = []
        for i in range(3):
            temp_effect = (self.base_temperature - 20.0) * 0.008
            wear_effect = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3][i] * 0.3
            noise = random.gauss(0, 0.015) * self.noise_multiplier
            c = (bases[i] * self.backlash_multiplier) + temp_effect + wear_effect + noise
            clearances.append(max(0.0, c))
        return tuple(clearances)

    def _generate_star_position(self) -> tuple:
        ra_target = random.uniform(0, 360)
        dec_target = random.uniform(-70, 70)

        total_gear_error = sum([
            self.gear_wear_1 * 0.3,
            self.gear_wear_2 * 0.3,
            self.gear_wear_3 * 0.3,
        ])

        ra_error_arcmin = random.gauss(0.3 + total_gear_error * 5.0, 0.15 + total_gear_error) * self.noise_multiplier
        dec_error_arcmin = random.gauss(0.25 + total_gear_error * 4.0, 0.12 + total_gear_error * 0.8) * self.noise_multiplier

        dec_cos = max(0.01, math.cos(dec_target * DEG_TO_RAD))
        ra_observed = ra_target + ra_error_arcmin / DEG_TO_ARCMIN / dec_cos
        dec_observed = dec_target + dec_error_arcmin / DEG_TO_ARCMIN

        ra_observed = ra_observed % 360
        dec_observed = max(-90, min(90, dec_observed))

        return (
            ra_observed, dec_observed,
            ra_target, dec_target,
            ra_error_arcmin, dec_error_arcmin
        )

    def _compute_cumulative_error(self, gear_errors, bearing_clearances) -> float:
        cumulative = 0.0
        wears = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3]
        for i in range(3):
            cumulative += gear_errors[i] * (1.0 + wears[i] * 2.0)
            cumulative += bearing_clearances[i] * 0.5
        cumulative += (self.base_temperature - 20.0) * 0.02
        return cumulative

    def _update_wear_levels(self):
        rate = self.wear_rate
        self.gear_wear_1 = min(0.99, self.gear_wear_1 + random.uniform(0, 0.00005) * rate)
        self.gear_wear_2 = min(0.99, self.gear_wear_2 + random.uniform(0, 0.00004) * rate)
        self.gear_wear_3 = min(0.99, self.gear_wear_3 + random.uniform(0, 0.000045) * rate)

        if self.tick_count % 1440 == 0 and random.random() < 0.05 * rate:
            spike = random.uniform(0.02, 0.08) * rate
            self.gear_wear_1 = min(0.99, self.gear_wear_1 + spike)
            print(f"[WARN] Gear 1 wear spike detected, current wear: {self.gear_wear_1:.4f}")

    def generate_reading(self) -> Dict[str, Any]:
        self.tick_count += 1

        azimuth, elevation, equatorial = self._generate_angles()
        gear_err_1, gear_err_2, gear_err_3 = self._generate_gear_meshing_errors()
        brg_clear_1, brg_clear_2, brg_clear_3 = self._generate_bearing_clearances()
        obs_ra, obs_dec, theo_ra, theo_dec, ra_dev, dec_dev = self._generate_star_position()

        temp = self.base_temperature + random.gauss(0, 0.8) * self.noise_multiplier + 2.0 * math.sin(self.tick_count / 720.0)
        humidity = max(20.0, min(95.0, self.base_humidity + random.gauss(0, 3.0) * self.noise_multiplier - 10.0 * math.sin(self.tick_count / 720.0)))

        cumulative = self._compute_cumulative_error(
            [gear_err_1, gear_err_2, gear_err_3],
            [brg_clear_1, brg_clear_2, brg_clear_3]
        )

        self._update_wear_levels()

        return {
            "timestamp": datetime.datetime.utcnow().isoformat() + "Z",
            "device_id": self.device_id,
            "axis_azimuth_angle": round(azimuth, 6),
            "axis_elevation_angle": round(elevation, 6),
            "axis_equatorial_angle": round(equatorial, 6),
            "gear_meshing_error_1": round(gear_err_1, 6),
            "gear_meshing_error_2": round(gear_err_2, 6),
            "gear_meshing_error_3": round(gear_err_3, 6),
            "bearing_clearance_1": round(brg_clear_1, 6),
            "bearing_clearance_2": round(brg_clear_2, 6),
            "bearing_clearance_3": round(brg_clear_3, 6),
            "observed_star_ra": round(obs_ra, 6),
            "observed_star_dec": round(obs_dec, 6),
            "theoretical_ra": round(theo_ra, 6),
            "theoretical_dec": round(theo_dec, 6),
            "ra_deviation": round(ra_dev, 6),
            "dec_deviation": round(dec_dev, 6),
            "cumulative_transmission_error": round(cumulative, 6),
            "gear_wear_level_1": round(self.gear_wear_1, 6),
            "gear_wear_level_2": round(self.gear_wear_2, 6),
            "gear_wear_level_3": round(self.gear_wear_3, 6),
            "temperature": round(temp, 3),
            "humidity": round(humidity, 3),
        }

    def send_via_mqtt(self, reading: Dict[str, Any]) -> bool:
        if not self.mqtt_client:
            return False
        try:
            payload = json.dumps(reading)
            result = self.mqtt_client.publish(self.mqtt_topic, payload, qos=1)
            if result.rc == 0:
                return True
            else:
                print(f"[MQTT ERROR] Publish failed, rc={result.rc}")
                return False
        except Exception as e:
            print(f"[MQTT ERROR] {e}")
            return False

    def send_via_http(self, reading: Dict[str, Any]) -> bool:
        try:
            resp = requests.post(
                self.endpoint,
                json=reading,
                headers={"Content-Type": "application/json"},
                timeout=10
            )
            if resp.status_code == 200:
                data = resp.json()
                return True
            else:
                print(f"[ERROR] HTTP {resp.status_code}: {resp.text[:200]}")
                return False
        except Exception as e:
            print(f"[ERROR] Failed to send reading: {e}")
            return False

    def send_reading(self, reading: Dict[str, Any]) -> bool:
        if self.mqtt_client:
            ok = self.send_via_mqtt(reading)
            if not ok:
                return self.send_via_http(reading)
            return ok
        else:
            return self.send_via_http(reading)

    def run(self, interval_seconds: int = 60, max_iterations: int = -1):
        print(f"Starting Hunyi sensor simulator: device={self.device_id}")
        if self.mqtt_broker:
            print(f"  Transport: MQTT ({self.mqtt_broker}, topic={self.mqtt_topic})")
        else:
            print(f"  Transport: HTTP ({self.endpoint})")
        print(f"  Report interval: {interval_seconds}s")
        print(f"  Initial gear wear: 1={self.gear_wear_1:.4f}, 2={self.gear_wear_2:.4f}, 3={self.gear_wear_3:.4f}")
        print(f"  Wear rate: x{self.wear_rate:.2f}, Backlash: x{self.backlash_multiplier:.2f}, Noise: x{self.noise_multiplier:.2f}")
        print("=" * 80)

        count = 0
        try:
            while True:
                reading = self.generate_reading()
                ts = reading["timestamp"]
                cumulative = reading["cumulative_transmission_error"]
                ra_dev = reading["ra_deviation"]
                dec_dev = reading["dec_deviation"]
                status = self.send_reading(reading)
                status_str = "OK" if status else "FAIL"

                print(
                    f"[{ts[:19]}] #{count} err={cumulative:.3f}' "
                    f"ΔRA={ra_dev:+.3f}' ΔDec={dec_dev:+.3f}' "
                    f"gear1={reading['gear_wear_level_1']:.3f} -> {status_str}"
                )

                count += 1
                if max_iterations > 0 and count >= max_iterations:
                    print(f"Reached max iterations ({max_iterations}), stopping.")
                    break

                time.sleep(interval_seconds)

        except KeyboardInterrupt:
            print("\nSimulator stopped by user.")
            print(f"Final gear wear: 1={self.gear_wear_1:.4f}, 2={self.gear_wear_2:.4f}, 3={self.gear_wear_3:.4f}")
        finally:
            if self.mqtt_client:
                self.mqtt_client.loop_stop()
                self.mqtt_client.disconnect()


def main():
    parser = argparse.ArgumentParser(description="浑仪传感器模拟器")
    parser.add_argument("--device", default="HUNYI-001", help="设备ID (default: HUNYI-001)")
    parser.add_argument("--api", default="http://backend:8080", help="后端API地址 (default: http://backend:8080)")
    parser.add_argument("--mqtt", default=None, help="MQTT broker地址 (host:port)，如: mqtt:1883")
    parser.add_argument("--mqtt-topic", default="hunyi/sensor", help="MQTT主题 (default: hunyi/sensor)")
    parser.add_argument("--interval", type=int, default=60, help="上报间隔（秒）(default: 60)")
    parser.add_argument("--count", type=int, default=-1, help="最大上报次数，-1为无限 (default: -1)")
    parser.add_argument("--fast", action="store_true", help="快速模式：1秒间隔")

    parser.add_argument("--wear-1", type=float, default=0.10, help="齿轮组1初始磨损 (0-1, default: 0.10)")
    parser.add_argument("--wear-2", type=float, default=0.08, help="齿轮组2初始磨损 (0-1, default: 0.08)")
    parser.add_argument("--wear-3", type=float, default=0.09, help="齿轮组3初始磨损 (0-1, default: 0.09)")
    parser.add_argument("--wear-rate", type=float, default=1.0, help="磨损增长倍率 (default: 1.0)")

    parser.add_argument("--backlash", type=float, default=1.0, help="齿轮间隙/啮合误差倍率 (default: 1.0)")
    parser.add_argument("--noise", type=float, default=1.0, help="噪声倍率 (default: 1.0)")

    parser.add_argument("--profile", choices=["normal", "worn", "broken", "cold", "hot"],
                        help="预设场景: normal(正常)/worn(严重磨损)/broken(损坏)/cold(低温)/hot(高温)")

    args = parser.parse_args()

    interval = 1 if args.fast else args.interval

    if args.profile:
        profiles = {
            "normal": {"wear": (0.10, 0.08, 0.09), "wear_rate": 1.0, "backlash": 1.0, "noise": 1.0},
            "worn":   {"wear": (0.70, 0.65, 0.68), "wear_rate": 2.0, "backlash": 2.5, "noise": 2.0},
            "broken": {"wear": (0.95, 0.90, 0.92), "wear_rate": 5.0, "backlash": 5.0, "noise": 4.0},
            "cold":   {"wear": (0.10, 0.08, 0.09), "wear_rate": 0.5, "backlash": 1.5, "noise": 1.5},
            "hot":    {"wear": (0.15, 0.12, 0.14), "wear_rate": 2.0, "backlash": 2.0, "noise": 2.0},
        }
        p = profiles[args.profile]
        print(f"[PROFILE] Using '{args.profile}' preset: wear={p['wear']}, backlash=x{p['backlash']}")
        initial_wear = p["wear"]
        wear_rate = p["wear_rate"]
        backlash = p["backlash"]
        noise = p["noise"]
    else:
        initial_wear = (args.wear_1, args.wear_2, args.wear_3)
        wear_rate = args.wear_rate
        backlash = args.backlash
        noise = args.noise

    sim = HunyiSensorSimulator(
        device_id=args.device,
        api_url=args.api,
        mqtt_broker=args.mqtt,
        mqtt_topic=args.mqtt_topic,
        initial_wear=initial_wear,
        wear_rate=wear_rate,
        backlash_multiplier=backlash,
        noise_multiplier=noise,
    )
    sim.run(interval_seconds=interval, max_iterations=args.count)


if __name__ == "__main__":
    main()
