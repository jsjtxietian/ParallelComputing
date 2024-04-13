class CownBase : StableOrder
{
    volatile Request? last = null;
}


class Request
{
    volatile Behaviour? next = null;
    volatile bool scheduled = false;
    CownBase target;

    Request(CownBase t) { target = t; }

    void StartAppendReq(Behaviour behaviour)
    {
        var prev = Exchange(ref target.last, this);
        if (prev == null)
        {
            behaviour.ResolveOne();
            return;
        }
        while (!prev.scheduled) { /* spin */ }
        prev.next = behaviour;
    }

    void FinishAppendReq() { scheduled = true; }

    void Release()
    {
        if (next == null)
        {
            if (CompareExchange(ref target.last, null, this) == this)
                return;
            while (next == null) { /* spin */ }
        }
        next.ResolveOne();
    }
}

class Behaviour
{
    Action thunk;
    int count;
    Request[] requests;

    Behaviour(Action t, CownBase[] cowns)
    {
        thunk = t;
        requests = new Request[cowns.Length];
        for (int i = 0; i < cowns.Length; i++)
            requests[i] = new Request(cowns[i]);
    }

    static void Schedule(Action t, params CownBase[] cowns)
    {
        Array.Sort(cowns);
        var behaviour = new Behaviour(t, cowns);
        behaviour.count = cowns.Length + 1;
        foreach (var r in behaviour.requests)
            r.StartAppendReq(behaviour);
        foreach (var r in behaviour.requests)
            r.FinishAppendReq();
        behaviour.ResolveOne();
    }

    void ResolveOne()
    {
        if (Decrement(ref count) != 0)
            return;
        Task.Run(() =>
        {
            thunk();
            foreach (var r in requests)
                r.Release();
        });
    }
}