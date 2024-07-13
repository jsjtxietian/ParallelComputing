/*******************************************************************************
 * Copyright 2019 Intel Corporation
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files(the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and / or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions :
 * 
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 ******************************************************************************/
#pragma once

#include "gts/micro_scheduler/WorkerPool.h"
#include "gts/micro_scheduler/MicroScheduler.h"
#include "gts/micro_scheduler/patterns/ParallelReduce.h"
#include "gts/micro_scheduler/patterns/Partitioners.h"
#include "gts/micro_scheduler/patterns/Range1d.h"

using namespace gts;

namespace gts_examples {

//------------------------------------------------------------------------------
void fullParallelReduce()
{
    printf ("================\n");
    printf ("fullParallelReduce\n");
    printf ("================\n");

    // Init boilerplate
    WorkerPool workerPool;
    bool result = workerPool.initialize(1);
    GTS_ASSERT(result);
    MicroScheduler microScheduler;
    result = microScheduler.initialize(&workerPool);
    GTS_ASSERT(result);

    size_t const elementCount = 1 << 2;

    // Create the array to reduce.
    std::vector<uint32_t> vec(elementCount);
    for (size_t ii = 0; ii < elementCount; ++ii)
    {
        vec[ii] = uint32_t(ii);
    }

    // Make a parallel-reduce object for this scheduler. We do this because
    // there can be multiple scheduler objects.
    ParallelReduce parallelReduce(microScheduler);

    auto partitionerType = AdaptivePartitioner();

    uint32_t reduction = parallelReduce(

        // The 1D iterator range parallel-for will iterate over.
        Range1d<std::vector<uint32_t>::iterator>(vec.begin(), vec.end(), 1),

        // The function parallel-reduce will execute on each block of the range.
        // It returns the reduction of the block.
        [](Range1d<std::vector<uint32_t>::iterator>& range, void*, TaskContext const&) -> uint32_t
        {
            uint32_t result = 0;
            for (auto ii = range.begin(); ii != range.end(); ++ii)
            {
                result += *ii;
            }
            return result;
        },

        // The function that combines the block reductions.
        [](uint32_t const& lhs, uint32_t const& rhs, void*, TaskContext const&) -> uint32_t
        {
            return lhs + rhs;
        },

        // The initial value of the reduction.
        0,

        // The partitioner object.
        partitionerType);

    microScheduler.shutdown();
    workerPool.shutdown();
}

} // namespace gts_examples
