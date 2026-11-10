; Outdoors, where the run began: locations of the player, a created marker,
; a dropped item and a far map marker, and GetCrimeGold with no faction in
; a hold, kept short (a guard could come to arrest). Skipped when the run
; began indoors.

String Function TestOutdoorLocations()
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    ObjectReference s = Player.PlaceAtMe(IronSword, 1, false, false)
    Utility.Wait(1.0)
    Location here = Player.GetCurrentLocation()
    String r = "(Whiterun Hold " + Id(WhiterunHold) + ", Whiterun " + Id(WhiterunCity) + ", Bleak Falls " + Id(BleakFalls) + ")\n"
    r += "the player: current " + Id(here) + ", editor " + Id(Player.GetEditorLocation()) + ", in Whiterun Hold " + Player.IsInLocation(WhiterunHold)
    r += "; the start marker (created): current " + Id(StartMarker.GetCurrentLocation()) + ", editor " + Id(StartMarker.GetEditorLocation()) + "\n"
    r += "Whiterun's map marker (plugin, far): current " + Id(WhiterunMarker.GetCurrentLocation()) + ", editor " + Id(WhiterunMarker.GetEditorLocation())
    r += "; Solitude's: current " + Id(SolitudeMarker.GetCurrentLocation()) + ", editor " + Id(SolitudeMarker.GetEditorLocation()) + "\n"
    r += "a created sword at the player: current " + Id(s.GetCurrentLocation()) + ", editor " + Id(s.GetEditorLocation())
    Player.AddItem(s, 1, true)
    Utility.Wait(1.0)
    r += "; picked up by AddItem(ref): current " + Id(s.GetCurrentLocation()) + ", editor " + Id(s.GetEditorLocation()) + "\n"
    String hereLoaded = "(none)"
    If here != None
        hereLoaded = "" + here.IsLoaded()
    EndIf
    r += "IsLoaded: here " + hereLoaded + ", Whiterun Hold " + WhiterunHold.IsLoaded() + ", Whiterun " + WhiterunCity.IsLoaded() + ", Bleak Falls " + BleakFalls.IsLoaded() + "\n"
    Player.RemoveItem(IronSword, 1, true, None)
    Int wr = CrimeWhiterun.GetCrimeGold()
    Int rift = CrimeRift.GetCrimeGold()
    CrimeWhiterun.SetCrimeGold(7)
    CrimeRift.SetCrimeGold(3)
    r += "bounties Whiterun 7, the Rift 3 (were " + wr + ", " + rift + "): GetCrimeGold with no faction =="
    ProbeValue.SetValue(0.0)
    r += " 0:" + Probe(Player, PS_CrimeGold, PE_CrimeGold)
    ProbeValue.SetValue(3.0)
    r += " 3:" + Probe(Player, PS_CrimeGold, PE_CrimeGold)
    ProbeValue.SetValue(7.0)
    r += " 7:" + Probe(Player, PS_CrimeGold, PE_CrimeGold)
    CrimeWhiterun.SetCrimeGold(wr)
    CrimeRift.SetCrimeGold(rift)
    return r + "\n"
EndFunction
