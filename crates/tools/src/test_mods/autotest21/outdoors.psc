; Outdoors, from where the run began (StartMarker): weather, fast travel,
; map markers. Skipped when the run began indoors.

String Function TestBackOutside()
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    String r = "back where the run began: interior " + Player.IsInInterior()
    r += "; the weather seen from the test cell earlier: " + Id(CellWeather) + ", sky mode there " + CellSkyMode
    r += "; here " + Id(Weather.GetCurrentWeather()) + ", sky mode " + Weather.GetSkyMode() + "\n"
    return r
EndFunction

; weather.md: transitions of three weathers with different deltas, with
; and without acceleration.
String Function Transition(Weather w, Float delta, Bool accel)
    Float r0 = Now()
    Float g0 = Utility.GetCurrentGameTime()
    w.SetActive(true, accel)
    Float half = -1.0
    Float halfGame = -1.0
    While Now() - r0 < 60.0 && Weather.GetCurrentWeatherTransition() < 1.0
        If half < 0.0 && Weather.GetCurrentWeather() == w && Weather.GetCurrentWeatherTransition() >= 0.5
            half = Now() - r0
            halfGame = (Utility.GetCurrentGameTime() - g0) * 1440.0
        EndIf
        Utility.Wait(0.1)
    EndWhile
    String s = "delta " + delta + " accelerate " + accel + ": current " + (Weather.GetCurrentWeather() == w) + ", half at " + Num(half) + " s / " + Num(halfGame) + " game min, full at " + Num(Now() - r0) + " s / " + Num((Utility.GetCurrentGameTime() - g0) * 1440.0) + " game min\n"
    return s
EndFunction

String Function TestWeather()
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    String r = "TimeScale " + TimeScale.GetValue() + "\n"
    WeatherA.ForceActive(true)
    Utility.Wait(2.0)
    r += Transition(WeatherB, DeltaB, false)
    r += Transition(WeatherC, DeltaC, false)
    r += Transition(WeatherA, DeltaA, false)
    r += Transition(WeatherC, DeltaC, true)
    Weather.ReleaseOverride()
    Int k = 0
    While k < 4
        r += "FindWeather(" + k + "):"
        Int n = 0
        While n < 6
            r += " " + Id(Weather.FindWeather(k))
            n += 1
        EndWhile
        r += "\n"
        k += 1
    EndWhile
    r += "(A " + Id(WeatherA) + ", B " + Id(WeatherB) + ", C " + Id(WeatherC) + ")\n"
    return r
EndFunction

; fast-travel.md: the gate after MoveTo; time taken.
String Function TestFastTravel()
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    Game.EnableFastTravel(false)
    String r = "EnableFastTravel(false): enabled " + Game.IsFastTravelEnabled()
    Player.MoveTo(StartMarker, 100.0, 0.0, 0.0, true)
    Utility.Wait(1.0)
    r += "; after a script MoveTo: " + Game.IsFastTravelEnabled() + "\n"
    Game.EnableFastTravel(true)
    Float g0 = Utility.GetCurrentGameTime()
    Float d = Player.GetDistance(WhiterunMarker)
    Game.FastTravel(WhiterunMarker)
    Utility.Wait(8.0)
    r += "Game.FastTravel to Whiterun's marker from " + Num(d) + " units: took " + Num((Utility.GetCurrentGameTime() - g0) * 24.0) + " game hours (less the 8 s waited: " + Num((Utility.GetCurrentGameTime() - g0) * 24.0 - 8.0 * TimeScale.GetValue() / 3600.0) + "), now " + Num(Player.GetDistance(WhiterunMarker)) + " from it\n"
    g0 = Utility.GetCurrentGameTime()
    d = Player.GetDistance(SolitudeMarker)
    Game.FastTravel(SolitudeMarker)
    Utility.Wait(8.0)
    r += "then to Solitude's from " + Num(d) + ": " + Num((Utility.GetCurrentGameTime() - g0) * 24.0 - 8.0 * TimeScale.GetValue() / 3600.0) + " game hours\n"
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    return r
EndFunction

; map-markers.md: a marker added to the map with travel, then come near.
String Function TestMapMarkers()
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    ObjectReference m = None
    Int i = 0
    While i < RemoteMarkers.GetSize() && m == None
        ObjectReference c = RemoteMarkers.GetAt(i) as ObjectReference
        If !c.IsMapMarkerVisible()
            m = c
        EndIf
        i += 1
    EndWhile
    If m == None
        return "every candidate marker was already on the map\n"
    EndIf
    String r = "marker " + Id(m) + ": visible " + m.IsMapMarkerVisible() + ", can travel " + m.CanFastTravelToMarker() + ", Locations Discovered " + Game.QueryStat("Locations Discovered")
    Float[] dist = new Float[6]
    dist[0] = 8000.0
    dist[1] = 4000.0
    dist[2] = 2000.0
    dist[3] = 1000.0
    dist[4] = 500.0
    dist[5] = 200.0
    i = 0
    While i < 6
        Player.MoveTo(m, dist[i], 0.0, 200.0, false)
        Utility.Wait(4.0)
        r += "\n  " + dist[i] + " off: visible " + m.IsMapMarkerVisible() + ", can travel " + m.CanFastTravelToMarker() + ", discovered " + Game.QueryStat("Locations Discovered")
        i += 1
    EndWhile
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    ObjectReference n = None
    i = 0
    While i < RemoteMarkers.GetSize() && n == None
        ObjectReference c2 = RemoteMarkers.GetAt(i) as ObjectReference
        If !c2.IsMapMarkerVisible()
            n = c2
        EndIf
        i += 1
    EndWhile
    If n != None
        n.AddToMap(true)
        r += "\nanother, AddToMap(true): visible " + n.IsMapMarkerVisible() + ", can travel " + n.CanFastTravelToMarker() + ", discovered " + Game.QueryStat("Locations Discovered")
        Player.MoveTo(n, 200.0, 0.0, 200.0, false)
        Utility.Wait(4.0)
        r += "; come within 200: discovered " + Game.QueryStat("Locations Discovered")
        Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
        Utility.Wait(5.0)
    EndIf
    return r + "\n"
EndFunction
