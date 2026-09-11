FIELDS = '''sim_time agl_ft ias_kias groundspeed_kt vvi_fpm heading_true_deg bank_deg pitch_deg beta_deg pitch_rate_raw normal_g throttle_ratio flap_handle_ratio elevator_trim_ratio flap_actual_ratio ground_any mass_kg engine_power_w fuel_flow_kg_s engine_rpm aileron_input elevator_input rudder_input vertical_speed_mps wind_speed_mps wind_direction_true_deg true_airspeed_mps native_slip_deg override_path runway_along_ft runway_cross_ft ground_track_true_deg elevation_msl_ft contact_latched first_sim_time first_along_ft first_cross_ft first_kias first_indicated_fpm first_physical_fpm first_pitch_deg first_normal_g first_local_wind_kt last_airborne_along_ft last_airborne_sim_time post_contact_max_g post_contact_max_agl_ft post_contact_air_frames predicted_cross_ft cross_accel_fps2 telemetry_ready telemetry_version phase_id sequence bank_command pitch_command flap_command throttle_command turn_lead_ft desired_vertical_fps vertical_accel_fps2 wind_pitch_rate_ff roundout_pitch_rate_command control_dt_s cut_sim_time roundout_sim_time reason_id native_running configured native_version entry_gate_s config_token native_steps heartbeat_age_s run_token'''.split()
PHASES = ['idle', 'ready', 'downwind', 'delay', 'turn_to_base', 'base', 'turn_to_final', 'final', 'rollout', 'complete', 'aborted']
REASONS = ['none', 'entry_gate', 'low_before_alignment', 'envelope', 'sim_timeout', 'supervisor_lost',
           'invalid_configuration', 'native_frame_gap', 'wind_mismatch', 'mass_mismatch', 'override_conflict',
           'cancelled', 'missing_dataref', 'trace_error']

def decode(values):
    if len(values) != len(FIELDS):
        raise RuntimeError(f'Native protocol mismatch: {len(values)} fields, expected {len(FIELDS)}')
    result = dict(zip(FIELDS, values))
    result['phase'] = PHASES[int(result['phase_id'])]
    result['reason'] = REASONS[int(result['reason_id'])]
    return result
