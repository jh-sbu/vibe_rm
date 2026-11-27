; The test pens' helpers (`pens.rs`, `Batch { pens: true }`): the pens'
; markers by number, placing in a pen, the player to the booth, objects
; placed and swept up in the pens.

; Pen i's marker.
ObjectReference Function Pen(Int i)
    If i == 0
        return Pen0
    ElseIf i == 1
        return Pen1
    ElseIf i == 2
        return Pen2
    ElseIf i == 3
        return Pen3
    ElseIf i == 4
        return Pen4
    ElseIf i == 5
        return Pen5
    ElseIf i == 6
        return Pen6
    ElseIf i == 7
        return Pen7
    EndIf
    return Pen8
EndFunction

; An actor of base b placed in pen i, x / y from its centre.
Actor Function SpawnPen(ActorBase b, Int i, Float x, Float y)
    Actor a = Pen(i).PlaceActorAtMe(b, 4, None)
    a.MoveTo(Pen(i), x, y, 0.0, false)
    return a
EndFunction

Function GoPens()
    Player.MoveTo(PenBooth, 0.0, 0.0, 10.0, true)
    Utility.Wait(4.0)
EndFunction

; An object of base f placed in pen i, x / y / z from its centre.
ObjectReference Function PutPen(Form f, Int i, Float x, Float y, Float z)
    ObjectReference o = Pen(i).PlaceAtMe(f, 1, false, false)
    o.MoveTo(Pen(i), x, y, z, false)
    return o
EndFunction

; Deletes every reference of base f lying in the pens.
Function SweepPens(Form f)
    Int i = 0
    While i < 9
        ObjectReference w = Game.FindClosestReferenceOfTypeFromRef(f, Pen(i), 700.0)
        Int n = 0
        While w != None && n < 10
            Gone(w)
            Utility.Wait(0.1)
            w = Game.FindClosestReferenceOfTypeFromRef(f, Pen(i), 700.0)
            If w != None && w.IsDeleted()
                w = None
            EndIf
            n += 1
        EndWhile
        i += 1
    EndWhile
EndFunction
