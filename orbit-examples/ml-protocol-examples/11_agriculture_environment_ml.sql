CREATE TABLE IF NOT EXISTS crop_plots (
    plot_id SERIAL PRIMARY KEY,
    region VARCHAR(50),
    soil_ph FLOAT,
    soil_moisture FLOAT,
    rainfall_mm FLOAT,
    temperature_c FLOAT,
    sunlight_hours FLOAT,
    yield_kg FLOAT
);

INSERT INTO crop_plots (region, soil_ph, soil_moisture, rainfall_mm, temperature_c, sunlight_hours, yield_kg)
VALUES
    ('North', 6.5, 0.30, 120.0, 20.0, 8.0, 1500.0),
    ('South', 7.2, 0.45, 90.0, 28.0, 10.0, 1700.0),
    ('East', 6.8, 0.25, 200.0, 18.0, 7.0, 1300.0);

SELECT ML_TRAIN_MODEL(
    'crop_yield_reg',
    'linear_regression',
    ARRAY[
        soil_ph,
        soil_moisture,
        rainfall_mm,
        temperature_c,
        sunlight_hours
    ],
    yield_kg
) FROM crop_plots;

SELECT ML_EVALUATE_MODEL(
    'crop_yield_reg',
    ARRAY[
        soil_ph,
        soil_moisture,
        rainfall_mm,
        temperature_c,
        sunlight_hours
    ],
    yield_kg
) FROM crop_plots;

SELECT 
    plot_id,
    ML_PREDICT(
        'crop_yield_reg',
        ARRAY[
            soil_ph,
            soil_moisture,
            rainfall_mm,
            temperature_c,
            sunlight_hours
        ]
    ) AS predicted_yield
FROM crop_plots
ORDER BY predicted_yield DESC;
