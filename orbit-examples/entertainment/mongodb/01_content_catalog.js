// MongoDB: Content Management System (CMS)
// Flexible schema for managing Movies, Series, and localization assets.

// 1. Insert a Movie Document
// Rich metadata including nested arrays for cast, ratings, and localized assets.
db.content_catalog.insertOne({
    "content_id": "mv_88392",
    "title": "Interstellar_Drift",
    "type": "Movie",
    "release_year": 2024,
    "genre": ["Sci-Fi", "Drama", "Adventure"],
    "runtime_minutes": 145,
    "maturity_rating": "PG-13",
    "synopsis": "A pilot must navigate a collapsing nebula to save a lost colony.",
    "cast": [
        { "name": "Sarah Connor", "role": "Commander" },
        { "name": "John Doe", "role": "Navigator" }
    ],
    "ratings": {
        "imdb": 8.5,
        "rotten_tomatoes": 92,
        "metacritic": 88
    },
    // Multi-region assets
    "assets": [
        {
            "region": "US",
            "title_localized": "Interstellar Drift",
            "audio_tracks": ["en-US", "es-US"],
            "subtitles": ["en", "es", "fr"],
            "poster_url": "s3://assets/mv_88392/us_poster.jpg"
        },
        {
            "region": "JP",
            "title_localized": "Hoshi no Drift",
            "audio_tracks": ["ja-JP", "en-US"],
            "subtitles": ["ja", "en"],
            "poster_url": "s3://assets/mv_88392/jp_poster.jpg"
        }
    ],
    "available_resolutions": ["4K", "1080p", "720p"],
    "created_at": new Date()
});

// 2. Insert a TV Series Document
// Hierarchical structure for Seasons and Episodes
db.content_catalog.insertOne({
    "content_id": "tv_22910",
    "title": "Silicon Valley Shadows",
    "type": "Series",
    "seasons": [
        {
            "season_number": 1,
            "release_year": 2023,
            "episodes": [
                {
                    "episode_number": 1,
                    "title": "The Angel Investor",
                    "runtime_minutes": 45,
                    "plot": "The team pitches to a mysterious billionaire."
                },
                {
                    "episode_number": 2,
                    "title": "Beta Testing",
                    "runtime_minutes": 42,
                    "plot": "A bug in the code threatens the launch."
                }
            ]
        }
    ],
    "showrunner": "Mike Judge_ish",
    "status": "Ongoing"
});

// 3. Query: Find 4K Sci-Fi Movies
// Searching within nested arrays and fields
db.content_catalog.find({
    "type": "Movie",
    "genre": "Sci-Fi",
    "available_resolutions": "4K"
}).projection({ "title": 1, "release_year": 1 });

// 4. Update: Add a new subtitle language for a specific region
db.content_catalog.updateOne(
    { "content_id": "mv_88392", "assets.region": "US" },
    { $push: { "assets.$.subtitles": "pt-BR" } }
);
